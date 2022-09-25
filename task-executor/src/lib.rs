use std::fmt::Debug;
use std::hash::Hash;
use std::sync::atomic::{AtomicIsize, Ordering};

use futures::channel::mpsc;
use once_cell::sync::OnceCell;

pub use exec::Executor;
pub use spawner::Spawner;

mod exec;
mod spawner;

impl<T: ?Sized> SpawnExt for T where T: futures::Future {}

pub trait SpawnExt: futures::Future {
    fn spawn<G>(self, exec: &Executor<G>) -> Spawner<Self, G>
    where
        Self: Sized + Send + 'static,
        Self::Output: Send + 'static,
        G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        let f = Spawner::new(exec, self);
        assert_future::<_, _>(f)
    }
}

impl<T: ?Sized> SpawnDefaultExt for T where T: futures::Future {}

pub trait SpawnDefaultExt: futures::Future {
    fn spawn(self) -> Spawner<'static, Self, ()>
    where
        Self: Sized + Send + 'static,
        Self::Output: Send + 'static,
    {
        let f = Spawner::new(default(), self);
        assert_future::<_, _>(f)
    }
}

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

    #[inline]
    pub fn group(self) -> GroupBuilder {
        GroupBuilder { builder: self }
    }

    pub fn build(self) -> (Executor, impl futures::Future<Output = ()>) {
        Executor::new(self.workers, self.queue_max)
    }
}

pub struct GroupBuilder {
    builder: Builder,
}

impl GroupBuilder {
    pub fn build<G>(self) -> (Executor<G>, impl futures::Future<Output = ()>)
    where
        G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
    {
        Executor::new(self.builder.workers, self.builder.queue_max)
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

#[derive(thiserror::Error, Debug)]
pub enum Error<T> {
    #[error("send error")]
    SendError(ErrorType<T>),
    #[error("try send error")]
    TrySendError(ErrorType<T>),
    #[error("send timeout error")]
    SendTimeoutError(ErrorType<T>),
    #[error("recv result error")]
    RecvResultError,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorType<T> {
    Full(Option<T>),
    Closed(Option<T>),
    Timeout(Option<T>),
}

impl<T> Error<T> {
    pub fn is_full(&self) -> bool {
        matches!(
            self,
            Error::SendError(ErrorType::Full(_))
                | Error::TrySendError(ErrorType::Full(_))
                | Error::SendTimeoutError(ErrorType::Full(_))
        )
    }

    pub fn is_closed(&self) -> bool {
        matches!(
            self,
            Error::SendError(ErrorType::Closed(_))
                | Error::TrySendError(ErrorType::Closed(_))
                | Error::SendTimeoutError(ErrorType::Closed(_))
        )
    }

    pub fn is_timeout(&self) -> bool {
        matches!(
            self,
            Error::SendError(ErrorType::Timeout(_))
                | Error::TrySendError(ErrorType::Timeout(_))
                | Error::SendTimeoutError(ErrorType::Timeout(_))
        )
    }
}

impl<T> From<mpsc::TrySendError<T>> for Error<T> {
    fn from(e: mpsc::TrySendError<T>) -> Self {
        if e.is_full() {
            Error::TrySendError(ErrorType::Full(Some(e.into_inner())))
        } else {
            Error::TrySendError(ErrorType::Closed(Some(e.into_inner())))
        }
    }
}

impl<T> From<mpsc::SendError> for Error<T> {
    fn from(e: mpsc::SendError) -> Self {
        if e.is_full() {
            Error::SendError(ErrorType::Full(None))
        } else {
            Error::SendError(ErrorType::Closed(None))
        }
    }
}

// Just a helper function to ensure the futures we're returning all have the
// right implementations.
pub(crate) fn assert_future<T, F>(future: F) -> F
where
    F: futures::Future<Output = T>,
{
    future
}

static DEFAULT_EXECUTOR: OnceCell<Executor> = OnceCell::new();

pub fn set_default(exec: Executor) -> Result<(), Executor> {
    DEFAULT_EXECUTOR.set(exec)
}

pub fn init_default() -> impl futures::Future<Output = ()> {
    let (exec, runner) = Builder::default().workers(100).queue_max(100_000).build();
    DEFAULT_EXECUTOR.set(exec).ok().unwrap();
    runner
}

pub fn default() -> &'static Executor {
    DEFAULT_EXECUTOR
        .get()
        .expect("default executor must be set first")
}
