use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use futures::channel::mpsc;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;

pub use builder::{Builder, SpawnDefaultExt, SpawnExt};
pub use exec::{TaskExecQueue, TaskType};
pub use local::LocalTaskExecQueue;
pub use local::LocalTaskType;
pub use local_builder::{LocalBuilder, LocalSender, LocalSpawnExt};
pub use local_spawner::{LocalGroupSpawner, LocalSpawner, TryLocalGroupSpawner, TryLocalSpawner};
pub use spawner::{GroupSpawner, Spawner, TryGroupSpawner, TrySpawner};

mod builder;
mod close;
mod exec;
mod flush;
mod spawner;

mod local;
mod local_builder;
mod local_spawner;

#[derive(Clone, Debug)]
struct Counter(std::sync::Arc<AtomicIsize>);

impl Counter {
    #[inline]
    fn new() -> Self {
        Counter(std::sync::Arc::new(AtomicIsize::new(0)))
    }

    #[inline]
    fn inc(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    #[inline]
    fn dec(&self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }

    #[inline]
    fn value(&self) -> isize {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
struct IndexSet(Arc<RwLock<HashSet<usize, ahash::RandomState>>>);

impl IndexSet {
    #[inline]
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashSet::default())))
    }

    #[inline]
    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.0.read().len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.0.read().is_empty()
    }

    #[inline]
    fn insert(&self, v: usize) {
        self.0.write().insert(v);
    }

    #[inline]
    fn pop(&self) -> Option<usize> {
        let mut set = self.0.write();
        if let Some(idx) = set.iter().next().copied() {
            set.remove(&idx);
            Some(idx)
        } else {
            None
        }
    }
}

struct GroupTaskExecQueue<TT> {
    tasks: VecDeque<TT>,
    is_running: bool,
}

impl<TT> GroupTaskExecQueue<TT> {
    #[inline]
    fn new() -> Self {
        Self {
            tasks: VecDeque::default(),
            is_running: false,
        }
    }

    #[inline]
    fn push(&mut self, task: TT) {
        self.tasks.push_back(task);
    }

    #[inline]
    fn pop(&mut self) -> Option<TT> {
        if let Some(task) = self.tasks.pop_front() {
            Some(task)
        } else {
            self.set_running(false);
            None
        }
    }

    #[inline]
    fn set_running(&mut self, b: bool) {
        self.is_running = b;
    }

    #[inline]
    fn is_running(&self) -> bool {
        self.is_running
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
    #[inline]
    pub fn is_full(&self) -> bool {
        matches!(
            self,
            Error::SendError(ErrorType::Full(_))
                | Error::TrySendError(ErrorType::Full(_))
                | Error::SendTimeoutError(ErrorType::Full(_))
        )
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        matches!(
            self,
            Error::SendError(ErrorType::Closed(_))
                | Error::TrySendError(ErrorType::Closed(_))
                | Error::SendTimeoutError(ErrorType::Closed(_))
        )
    }

    #[inline]
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

static DEFAULT_EXEC_QUEUE: OnceCell<TaskExecQueue> = OnceCell::new();

pub fn set_default(queue: TaskExecQueue) -> Result<(), TaskExecQueue> {
    DEFAULT_EXEC_QUEUE.set(queue)
}

pub fn init_default() -> impl futures::Future<Output = ()> {
    let (queue, runner) = Builder::default().workers(100).queue_max(100_000).build();
    DEFAULT_EXEC_QUEUE.set(queue).ok().unwrap();
    runner
}

pub fn default() -> &'static TaskExecQueue {
    DEFAULT_EXEC_QUEUE
        .get()
        .expect("default task execution queue must be set first")
}

#[test]
fn test_index_set() {
    let set = IndexSet::new();
    set.insert(1);
    set.insert(10);
    set.insert(100);
    assert_eq!(set.len(), 3);
    assert!(matches!(set.pop(), Some(1) | Some(10) | Some(100)));
    assert_eq!(set.len(), 2);
    set.pop();
    set.pop();
    assert_eq!(set.len(), 0);
}
