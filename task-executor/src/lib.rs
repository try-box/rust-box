use std::sync::atomic::{AtomicIsize, Ordering};

pub use exec::Executor;

mod exec;

pub struct Builder {
    workers: usize,
    queue_max: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            workers: 100,
            queue_max: 100_000,
        }
    }
}

impl Builder {
    #[inline]
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers;
        self
    }

    #[inline]
    pub fn queue_max(mut self, queue_max: usize) -> Self {
        self.queue_max = queue_max;
        self
    }

    pub fn build<Item>(self) -> (Executor<Item>, impl std::future::Future<Output=()>)
        where
            Item: std::future::Future + Send + 'static,
    {
        Executor::new(self.workers, self.queue_max)
    }
}

#[derive(Clone, Debug)]
struct Counter(std::sync::Arc<AtomicIsize>);

impl Counter {
    fn new() -> Self {
        Counter(std::sync::Arc::new(AtomicIsize::new(0)))
    }

    fn inc(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn dec(&self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }

    fn value(&self) -> isize {
        self.0.load(Ordering::SeqCst)
    }
}
