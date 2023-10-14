use std::collections::HashSet;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

//use rust_box::counter::LocalCounter as Counter;
//use tokio::task::spawn_local as spawn;

use rust_box::counter::Counter;
use tokio::task::spawn;

#[allow(dead_code)]
fn time_now() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t)
        .unwrap_or_else(|_| Duration::from_millis(chrono::Local::now().timestamp_millis() as u64))
}

#[allow(dead_code)]
fn timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs())
        .unwrap_or_else(|_| chrono::Local::now().timestamp() as u64)
}

#[allow(dead_code)]
fn timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as i64)
        .unwrap_or_else(|_| chrono::Local::now().timestamp_millis())
}

#[allow(dead_code)]
fn timestamp_secs_f64() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs_f64())
        .unwrap_or_else(|_| chrono::Local::now().timestamp_millis() as f64 / 1000.0)
}

fn main() {
    std::env::set_var("RUST_LOG", "channel_test=info");
    env_logger::init();

    let runner = async move {
        test_counter().await;
        test_counter2().await;
    };

    let rt = tokio::runtime::Builder::new_current_thread()
    //let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        //.worker_threads(8)
        .build()
        .unwrap();

//    tokio::runtime::Runtime::new().unwrap().block_on(runner);
    tokio::task::LocalSet::new().block_on(&rt, runner);
}

async fn test_counter() {
    let c = Counter::new(Duration::from_secs(5));
    let c1 = c.clone();
    let threads = Arc::new(parking_lot::RwLock::new(HashSet::new()));
    for _ in 0..1000 {
        let c1 = c1.clone();
        let threads1 = threads.clone();
        spawn(async move {
            for _ in 0..10000000 {
                let c1 = c1.clone();
                let threads1 = threads1.clone();
                spawn(async move {
                    c1.inc().await;
                    threads1.write().insert(std::thread::current().id());
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                });
                tokio::time::sleep(Duration::ZERO).await;
            }
        });
    }

    let use_time = std::time::Instant::now();
    for _ in 0..8 {
        println!(
            "{:?}, count: {}, max: {}, total: {}, rate: {}, threads: {}",
            use_time.elapsed(),
            c.count().await,
            c.max().await,
            c.total().await,
            c.rate().await,
            threads.read().len()
        );
                tokio::time::sleep(Duration::from_secs(5)).await;
//        std::thread::sleep(Duration::from_secs(5));
    }
}

async fn test_counter2() {
    let c = Counter2::new(Duration::from_secs(5));
    let c1 = c.clone();
    let threads = Arc::new(parking_lot::RwLock::new(HashSet::new()));
    for _ in 0..1000 {
        let c1 = c1.clone();
        let threads1 = threads.clone();
        spawn(async move {
            for _ in 0..10000000 {
                let c1 = c1.clone();
                let threads1 = threads1.clone();
                spawn(async move {
                    c1.inc();
                    threads1.write().insert(std::thread::current().id());
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                });
                tokio::time::sleep(Duration::ZERO).await;
            }
        });
    }

    let use_time = std::time::Instant::now();
    for _ in 0..8 {
        println!(
            "{:?}, count: {}, max: {}, total: {}, rate: {}, threads: {}",
            use_time.elapsed(),
            c.count(),
            c.max(),
            c.total(),
            c.rate(),
            threads.read().len()
        );
                tokio::time::sleep(Duration::from_secs(5)).await;
//        std::thread::sleep(Duration::from_secs(5));
    }
}


use std::sync::atomic::{AtomicI64, AtomicIsize, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct Counter2(Arc<Inner>);

struct Rater {
    total: AtomicIsize,
    rate: AtomicIsize, //f64,
    //The total of the most recent statistical period.
    recent: AtomicIsize, //isize,
    //Rate statistics period
    period: i64,
    recent_time: AtomicI64, //timestamp_millis
}

struct Inner {
    curr: AtomicIsize,
    max: AtomicIsize,
    rater: Rater,
}

impl Counter2 {
    #[inline]
    pub fn new(period: Duration) -> Self {
        let inner = Inner {
            curr: AtomicIsize::new(0),
            max: AtomicIsize::new(0),
            rater: Rater {
                total: AtomicIsize::new(0),
                rate: AtomicIsize::new(0),
                recent: AtomicIsize::new(0),
                period: period.as_millis() as i64,
                recent_time: AtomicI64::new(timestamp_millis()),
            },
        };
        Self(Arc::new(inner))
    }

    #[inline]
    pub fn inc(&self) {
        self.incs(1);
    }

    #[inline]
    pub fn incs(&self, c: isize) {
        let inner = &self.0;
        {
            let prev = inner.curr.fetch_add(c, Ordering::SeqCst);
            inner.max.fetch_max(prev + c, Ordering::SeqCst);
        }
        {
            inner.rater.total.fetch_add(c, Ordering::SeqCst);
            let now = timestamp_millis();
            let elapsed = now - inner.rater.recent_time.load(Ordering::SeqCst); //inner.rater.now.elapsed();
            if elapsed >= inner.rater.period {
                let total = inner.rater.total.load(Ordering::SeqCst);
                let period_count = total - inner.rater.recent.load(Ordering::SeqCst);
                let rate = ((period_count as f64 / (elapsed as f64 / 1000.0)) * 100.0) as isize;
                inner.rater.rate.store(rate, Ordering::SeqCst);
                inner.rater.recent_time.store(now, Ordering::SeqCst);
                inner.rater.recent.store(total, Ordering::SeqCst);
            }
        }
    }

    #[inline]
    pub fn sets(&self, c: isize) {
        {
            self.0.curr.store(c, Ordering::SeqCst);
            self.0.max.fetch_max(c, Ordering::SeqCst);
        }
        {
            self.0.rater.total.store(c, Ordering::SeqCst);
        }
    }

    #[inline]
    pub fn dec(&self) {
        self.decs(1)
    }

    #[inline]
    pub fn decs(&self, c: isize) {
        self.0.curr.fetch_sub(c, Ordering::SeqCst);
    }

    #[inline]
    pub fn set_curr_min(&self, c: isize) {
        self.0.curr.fetch_min(c, Ordering::SeqCst);
    }

    #[inline]
    pub fn set_max_max(&self, max: isize) {
        self.0.max.fetch_max(max, Ordering::SeqCst);
    }

    #[inline]
    pub fn count(&self) -> isize {
        self.0.curr.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn max(&self) -> isize {
        self.0.max.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn total(&self) -> isize {
        self.0.rater.total.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn rate(&self) -> f64 {
        self.0.rater.rate.load(Ordering::SeqCst) as f64 / 100.0
    }

    #[inline]
    pub fn add(&self, other: &Self) {
        {
            self.0
                .curr
                .fetch_add(other.0.curr.load(Ordering::SeqCst), Ordering::SeqCst);
            self.0
                .max
                .fetch_add(other.0.max.load(Ordering::SeqCst), Ordering::SeqCst);
        }
        {
            self.0
                .rater
                .total
                .fetch_add(other.0.rater.total.load(Ordering::SeqCst), Ordering::SeqCst);
            self.0
                .rater
                .rate
                .fetch_add(other.0.rater.rate.load(Ordering::SeqCst), Ordering::SeqCst);
        }
    }

    #[inline]
    pub fn set(&self, other: &Self) {
        {
            self.0
                .curr
                .store(other.0.curr.load(Ordering::SeqCst), Ordering::SeqCst);
            self.0
                .max
                .store(other.0.max.load(Ordering::SeqCst), Ordering::SeqCst);
        }
        {
            self.0
                .rater
                .total
                .store(other.0.rater.total.load(Ordering::SeqCst), Ordering::SeqCst);
            self.0
                .rater
                .rate
                .store(other.0.rater.rate.load(Ordering::SeqCst), Ordering::SeqCst);
            self.0.rater.recent_time.store(
                other.0.rater.recent_time.load(Ordering::SeqCst),
                Ordering::SeqCst,
            );
            self.0.rater.recent.store(
                other.0.rater.recent.load(Ordering::SeqCst),
                Ordering::SeqCst,
            );
        }
    }
}
