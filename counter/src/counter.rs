use core::time::Duration;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use futures::lock::Mutex;

#[derive(Clone)]
#[cfg(any(feature = "count", feature = "rate"))]
pub struct LocalCounter(Rc<CounterInner>);

impl Deref for LocalCounter {
    type Target = Rc<CounterInner>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LocalCounter {
    #[inline]
    pub fn new(period: Duration) -> Self {
        Self(Rc::new(CounterInner::new_inner(period)))
    }

    #[inline]
    pub async fn serialize(&self) -> bincode::Result<Vec<u8>> {
        bincode::serialize(self.0 .0.lock().await.deref())
    }

    #[inline]
    pub async fn deserialize(bytes: &[u8]) -> bincode::Result<LocalCounter> {
        let inner = bincode::deserialize::<Inner>(bytes)?;
        Ok(LocalCounter(Rc::new(CounterInner(Mutex::new(inner)))))
    }
}

#[derive(Clone)]
#[cfg(any(feature = "count", feature = "rate"))]
pub struct Counter(Arc<CounterInner>);

impl Deref for Counter {
    type Target = Arc<CounterInner>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Counter {
    #[inline]
    pub fn new(period: Duration) -> Self {
        Self(Arc::new(CounterInner::new_inner(period)))
    }

    #[inline]
    pub async fn serialize(&self) -> bincode::Result<Vec<u8>> {
        bincode::serialize(self.0 .0.lock().await.deref())
    }

    #[inline]
    pub async fn deserialize(bytes: &[u8]) -> bincode::Result<Counter> {
        let inner = bincode::deserialize::<Inner>(bytes)?;
        Ok(Counter(Arc::new(CounterInner(Mutex::new(inner)))))
    }
}

#[cfg(any(feature = "count", feature = "rate"))]
pub struct CounterInner(Mutex<Inner>);

impl Deref for CounterInner {
    type Target = Mutex<Inner>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "rate")]
#[derive(Serialize, Deserialize)]
struct Rater {
    total: isize,
    rate: f64,
    //The total of the most recent statistical period.
    recent: isize,
    //Rate statistics period
    period: Duration,
    #[serde(skip, default = "Rater::now_default")]
    now: Instant,
}

#[cfg(feature = "rate")]
impl Rater {
    fn now_default() -> Instant {
        Instant::now()
    }
}

#[derive(Serialize, Deserialize)]
#[cfg(any(feature = "count", feature = "rate"))]
pub struct Inner {
    #[cfg(feature = "count")]
    curr: isize,
    #[cfg(feature = "count")]
    max: isize,
    #[cfg(feature = "rate")]
    rater: Rater,
}

#[cfg(any(feature = "count", feature = "rate"))]
impl CounterInner {
    #[inline]
    fn new_inner(period: Duration) -> Self {
        let inner = Inner {
            #[cfg(feature = "count")]
            curr: 0,
            #[cfg(feature = "count")]
            max: 0,
            #[cfg(feature = "rate")]
            rater: Rater {
                total: 0,
                rate: 0.0,
                now: Instant::now(),
                recent: 0,
                period,
            },
        };

        Self(Mutex::new(inner))
    }

    #[inline]
    pub async fn inc(&self) {
        self.incs(1).await;
    }

    #[inline]
    pub async fn incs(&self, c: isize) {
        let mut inner = self.0.lock().await;
        #[cfg(feature = "count")]
        {
            inner.curr += c;
            inner.max = inner.max.max(inner.curr);
        }
        #[cfg(feature = "rate")]
        {
            inner.rater.total += c;
            let elapsed = inner.rater.now.elapsed();
            if elapsed >= inner.rater.period {
                let period_count = inner.rater.total - inner.rater.recent;
                inner.rater.rate = period_count as f64 / elapsed.as_secs_f64();
                inner.rater.now = Instant::now();
                inner.rater.recent = inner.rater.total;
            }
        }
    }

    #[inline]
    pub async fn sets(&self, c: isize) {
        let mut inner = self.0.lock().await;
        #[cfg(feature = "count")]
        {
            inner.curr = c;
            inner.max = inner.max.max(inner.curr);
        }
        #[cfg(feature = "rate")]
        {
            inner.rater.total = c;
        }
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn dec(&self) {
        self.decs(1).await
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn decs(&self, c: isize) {
        let mut inner = self.0.lock().await;
        inner.curr -= c;
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn set_curr_min(&self, count: isize) {
        let mut inner = self.0.lock().await;
        inner.curr = inner.curr.min(count);
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn set_max_max(&self, max: isize) {
        let mut inner = self.0.lock().await;
        inner.max = inner.max.max(max);
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn count(&self) -> isize {
        self.0.lock().await.curr
    }

    #[inline]
    #[cfg(feature = "count")]
    pub async fn max(&self) -> isize {
        self.0.lock().await.max
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub async fn total(&self) -> isize {
        self.0.lock().await.rater.total
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub async fn rate(&self) -> f64 {
        self.0.lock().await.rater.rate
    }

    #[inline]
    pub async fn add(&self, other: &Self) {
        let mut inner = self.0.lock().await;
        let other = other.0.lock().await;
        #[cfg(feature = "count")]
        {
            inner.curr += other.curr;
            inner.max += other.max;
        }
        #[cfg(feature = "rate")]
        {
            inner.rater.total += other.rater.total;
            inner.rater.rate += other.rater.rate;
        }
    }

    #[inline]
    pub async fn set(&self, other: &Self) {
        let mut inner = self.0.lock().await;
        let other = other.0.lock().await;
        #[cfg(feature = "count")]
        {
            inner.curr = other.curr;
            inner.max = other.max;
        }
        #[cfg(feature = "rate")]
        {
            inner.rater.total = other.rater.total;
            inner.rater.rate = other.rater.rate;
            inner.rater.now = other.rater.now;
            inner.rater.recent = other.rater.recent;
            //inner.period = other.period;
        }
    }
}
