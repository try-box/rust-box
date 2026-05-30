use core::time::Duration;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;

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
    pub fn serialize(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_stdvec(self.0 .0.lock().deref())
    }

    #[inline]
    pub fn deserialize(bytes: &[u8]) -> Result<LocalCounter, postcard::Error> {
        let inner = postcard::from_bytes::<Inner>(bytes)?;
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
    #[cfg(feature = "rate")]
    #[inline]
    pub fn new(period: Duration) -> Self {
        Self(Arc::new(CounterInner::new_inner(period)))
    }

    #[cfg(not(feature = "rate"))]
    #[inline]
    pub fn new() -> Self {
        Self(Arc::new(CounterInner::new_inner(Duration::from_secs(3))))
    }

    #[inline]
    pub fn serialize(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_stdvec(self.0 .0.lock().deref())
    }

    #[inline]
    pub fn deserialize(bytes: &[u8]) -> Result<Counter, postcard::Error> {
        let inner = postcard::from_bytes::<Inner>(bytes)?;
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
    //auto update rater,
    auto_update: bool,
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
                auto_update: true,
            },
        };

        Self(Mutex::new(inner))
    }

    #[inline]
    pub fn inc(&self) {
        self.incs(1);
    }

    #[inline]
    pub fn incs(&self, c: isize) {
        let mut inner = self.0.lock();
        #[cfg(feature = "count")]
        {
            inner.curr += c;
            inner.max = inner.max.max(inner.curr);
        }
        #[cfg(feature = "rate")]
        {
            inner.rater.total += c;
            if inner.rater.auto_update {
                let elapsed = inner.rater.now.elapsed();
                if elapsed >= inner.rater.period {
                    let period_count = inner.rater.total - inner.rater.recent;
                    inner.rater.rate = period_count as f64 / elapsed.as_secs_f64();
                    inner.rater.now = Instant::now();
                    inner.rater.recent = inner.rater.total;
                }
            }
        }
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub fn close_auto_update(&self) {
        self.0.lock().rater.auto_update = false;
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub fn rate_update(&self) {
        let mut inner = self.0.lock();
        let elapsed = inner.rater.now.elapsed();
        if elapsed >= inner.rater.period {
            let period_count = inner.rater.total - inner.rater.recent;
            inner.rater.rate = period_count as f64 / elapsed.as_secs_f64();
            inner.rater.now = Instant::now();
            inner.rater.recent = inner.rater.total;
        }
    }

    #[inline]
    pub fn sets(&self, c: isize) {
        let mut inner = self.0.lock();
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
    pub fn dec(&self) {
        self.decs(1)
    }

    #[inline]
    #[cfg(feature = "count")]
    pub fn decs(&self, c: isize) {
        let mut inner = self.0.lock();
        inner.curr -= c;
    }

    #[inline]
    #[cfg(feature = "count")]
    pub fn set_curr_min(&self, count: isize) {
        let mut inner = self.0.lock();
        inner.curr = inner.curr.min(count);
    }

    #[inline]
    #[cfg(feature = "count")]
    pub fn set_max_max(&self, max: isize) {
        let mut inner = self.0.lock();
        inner.max = inner.max.max(max);
    }

    #[inline]
    #[cfg(feature = "count")]
    pub fn count(&self) -> isize {
        self.0.lock().curr
    }

    #[inline]
    #[cfg(feature = "count")]
    pub fn max(&self) -> isize {
        self.0.lock().max
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub fn total(&self) -> isize {
        self.0.lock().rater.total
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub fn rate(&self) -> f64 {
        self.0.lock().rater.rate
    }

    #[inline]
    pub fn add(&self, other: &Self) {
        let mut inner = self.0.lock();
        let other = other.0.lock();
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
    pub fn set(&self, other: &Self) {
        let mut inner = self.0.lock();
        let other = other.0.lock();
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Helper: create a Counter regardless of whether `rate` is active.
    #[cfg(feature = "count")]
    fn new_counter() -> Counter {
        #[cfg(feature = "rate")]
        {
            Counter::new(Duration::from_secs(3))
        }
        #[cfg(not(feature = "rate"))]
        {
            Counter::new()
        }
    }

    // ── Count tests ─────────────────────────────────────────────────────────
    #[cfg(feature = "count")]
    mod count_tests {
        use super::*;

        #[test]
        fn new_counter_default_value_is_zero() {
            let c = new_counter();
            assert_eq!(c.count(), 0);
            assert_eq!(c.max(), 0);
        }

        #[test]
        fn inc_increments_by_one() {
            let c = new_counter();
            c.inc();
            assert_eq!(c.count(), 1);
            c.inc();
            assert_eq!(c.count(), 2);
        }

        #[test]
        fn incs_increments_by_n() {
            let c = new_counter();
            c.incs(5);
            assert_eq!(c.count(), 5);
            c.incs(7);
            assert_eq!(c.count(), 12);
        }

        #[test]
        fn dec_decrements_by_one() {
            let c = new_counter();
            c.sets(10);
            c.dec();
            assert_eq!(c.count(), 9);
            c.dec();
            assert_eq!(c.count(), 8);
        }

        #[test]
        fn decs_decrements_by_n() {
            let c = new_counter();
            c.sets(100);
            c.decs(30);
            assert_eq!(c.count(), 70);
            c.decs(20);
            assert_eq!(c.count(), 50);
        }

        #[test]
        fn count_returns_current_value() {
            let c = new_counter();
            assert_eq!(c.count(), 0);
            c.sets(42);
            assert_eq!(c.count(), 42);
        }

        #[test]
        fn max_tracks_maximum_reached() {
            let c = new_counter();
            assert_eq!(c.max(), 0);
            c.sets(10);
            assert_eq!(c.max(), 10);
            c.sets(5);
            assert_eq!(c.count(), 5);
            assert_eq!(c.max(), 10);
            c.sets(20);
            assert_eq!(c.max(), 20);
        }

        #[test]
        fn sets_sets_to_specific_value() {
            let c = new_counter();
            c.sets(42);
            assert_eq!(c.count(), 42);
            c.sets(100);
            assert_eq!(c.count(), 100);
        }

        #[test]
        fn set_curr_min_sets_minimum() {
            let c = new_counter();
            c.sets(100);
            c.set_curr_min(50);
            assert_eq!(c.count(), 50);
            c.set_curr_min(75);
            assert_eq!(c.count(), 50);
        }

        #[test]
        fn set_max_max_sets_maximum_max() {
            let c = new_counter();
            c.sets(10);
            assert_eq!(c.max(), 10);
            c.set_max_max(20);
            assert_eq!(c.max(), 20);
            c.set_max_max(15);
            assert_eq!(c.max(), 20);
        }

        #[test]
        fn add_combines_two_counters() {
            let c1 = new_counter();
            let c2 = new_counter();
            c1.sets(10);
            c2.sets(20);
            c1.add(&c2);
            assert_eq!(c1.count(), 30);
            assert_eq!(c1.max(), 30);
        }

        #[test]
        fn set_copies_from_another_counter() {
            let c1 = new_counter();
            let c2 = new_counter();
            c1.sets(10);
            c2.sets(20);
            c2.set(&c1);
            assert_eq!(c2.count(), 10);
            assert_eq!(c2.max(), 10);
        }

        #[test]
        fn multiple_concurrent_increments() {
            let c = new_counter();
            let mut handles = vec![];
            for _ in 0..10 {
                let c = c.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        c.inc();
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
            assert_eq!(c.count(), 1000);
            assert_eq!(c.max(), 1000);
        }
    }

    // ── Rate tests ──────────────────────────────────────────────────────────
    #[cfg(feature = "rate")]
    mod rate_tests {
        use super::*;

        #[test]
        fn new_counter_creates_with_period() {
            let c = Counter::new(Duration::from_secs(5));
            assert_eq!(c.total(), 0);
            assert_eq!(c.rate(), 0.0);
        }

        #[test]
        fn incs_increases_total() {
            let c = Counter::new(Duration::from_secs(0));
            c.incs(10);
            assert_eq!(c.total(), 10);
            c.incs(5);
            assert_eq!(c.total(), 15);
        }

        #[test]
        fn total_returns_aggregate() {
            let c = Counter::new(Duration::from_secs(0));
            assert_eq!(c.total(), 0);
            c.incs(100);
            assert_eq!(c.total(), 100);
        }

        #[test]
        fn rate_returns_calculated_rate() {
            let c = Counter::new(Duration::ZERO);
            c.incs(50);
            let r = c.rate();
            assert!(r > 0.0);
        }

        #[test]
        fn rate_update_triggers_recalculation() {
            let c = Counter::new(Duration::ZERO);
            c.close_auto_update();
            c.incs(100);
            assert_eq!(c.rate(), 0.0);
            c.rate_update();
            let r = c.rate();
            assert!(r > 0.0);
        }

        #[test]
        fn close_auto_update_stops_auto_updates() {
            let c = Counter::new(Duration::ZERO);
            c.incs(10);
            let r1 = c.rate();
            c.close_auto_update();
            assert_eq!(c.rate(), r1);
        }
    }

    // ── Combined count + rate tests ──────────────────────────────────────────
    #[cfg(all(feature = "count", feature = "rate"))]
    mod combined_tests {
        use super::*;

        #[test]
        fn count_and_rate_work_simultaneously() {
            let c = Counter::new(Duration::ZERO);
            c.inc();
            c.inc();
            assert_eq!(c.count(), 2);
            assert_eq!(c.max(), 2);
            assert_eq!(c.total(), 2);
            assert!(c.rate() > 0.0);
        }

        #[test]
        fn sets_updates_both_count_fields_and_total() {
            let c = Counter::new(Duration::ZERO);
            c.sets(42);
            assert_eq!(c.count(), 42);
            assert_eq!(c.max(), 42);
            assert_eq!(c.total(), 42);
        }

        #[test]
        fn add_combines_both_counters_fully() {
            let c1 = Counter::new(Duration::ZERO);
            let c2 = Counter::new(Duration::ZERO);
            c1.sets(10);
            c2.sets(20);
            c1.add(&c2);
            assert_eq!(c1.count(), 30);
            assert_eq!(c1.max(), 30);
            assert_eq!(c1.total(), 30);
        }

        #[test]
        fn set_copies_entire_counter_state() {
            let c1 = Counter::new(Duration::ZERO);
            let c2 = Counter::new(Duration::ZERO);
            c1.sets(50);
            c2.set(&c1);
            assert_eq!(c2.count(), 50);
            assert_eq!(c2.max(), 50);
            assert_eq!(c2.total(), 50);
        }
    }

    // ── Serialization tests ─────────────────────────────────────────────────
    #[cfg(any(feature = "count", feature = "rate"))]
    mod serialization_tests {
        use super::*;

        fn make_counter() -> Counter {
            #[cfg(feature = "rate")]
            {
                Counter::new(Duration::from_secs(3))
            }
            #[cfg(not(feature = "rate"))]
            {
                Counter::new()
            }
        }

        fn make_local_counter() -> LocalCounter {
            LocalCounter::new(Duration::from_secs(3))
        }

        #[test]
        fn serialize_deserialize_counter_roundtrip() {
            let c = make_counter();
            // sets() works with any feature: with count it sets curr/max,
            // with rate it sets total too, with both it sets everything.
            c.sets(42);
            #[cfg(feature = "count")]
            {
                assert_eq!(c.count(), 42);
                assert_eq!(c.max(), 42);
            }
            #[cfg(feature = "rate")]
            {
                assert_eq!(c.total(), 42);
            }

            let bytes = c.serialize().unwrap();
            let deserialized = Counter::deserialize(&bytes).unwrap();

            #[cfg(feature = "count")]
            {
                assert_eq!(deserialized.count(), 42);
                assert_eq!(deserialized.max(), 42);
            }
            #[cfg(feature = "rate")]
            {
                assert_eq!(deserialized.total(), 42);
            }
        }

        #[test]
        fn serialize_deserialize_local_counter_roundtrip() {
            let c = make_local_counter();
            c.sets(99);
            #[cfg(feature = "count")]
            {
                assert_eq!(c.count(), 99);
                assert_eq!(c.max(), 99);
            }
            #[cfg(feature = "rate")]
            {
                assert_eq!(c.total(), 99);
            }

            let bytes = c.serialize().unwrap();
            let deserialized = LocalCounter::deserialize(&bytes).unwrap();

            #[cfg(feature = "count")]
            {
                assert_eq!(deserialized.count(), 99);
                assert_eq!(deserialized.max(), 99);
            }
            #[cfg(feature = "rate")]
            {
                assert_eq!(deserialized.total(), 99);
            }
        }

        #[test]
        fn empty_counter_serialize_deserialize_roundtrip() {
            let c = make_counter();
            let bytes = c.serialize().unwrap();
            let deserialized = Counter::deserialize(&bytes).unwrap();

            #[cfg(feature = "count")]
            {
                assert_eq!(deserialized.count(), 0);
                assert_eq!(deserialized.max(), 0);
            }
            #[cfg(feature = "rate")]
            {
                assert_eq!(deserialized.total(), 0);
                assert_eq!(deserialized.rate(), 0.0);
            }
        }
    }
}
