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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use futures::Future;

    use super::*;
    use crate::builder::{Builder, SpawnExt};
    use crate::local_builder::LocalBuilder;

    // ------------------------------------------------------------------
    // 1. Error type tests
    // ------------------------------------------------------------------
    #[test]
    fn test_error_type_variants() {
        let err = Error::<i32>::TrySendError(ErrorType::Full(Some(42)));
        assert!(err.is_full());
        assert!(!err.is_closed());
        assert!(!err.is_timeout());
        assert!(matches!(
            err,
            Error::TrySendError(ErrorType::Full(Some(42)))
        ));

        let err = Error::<i32>::SendError(ErrorType::Full(Some(42)));
        assert!(err.is_full());
        assert!(matches!(err, Error::SendError(ErrorType::Full(Some(42)))));

        let err = Error::<i32>::SendTimeoutError(ErrorType::Full(Some(42)));
        assert!(err.is_full());
        assert!(matches!(
            err,
            Error::SendTimeoutError(ErrorType::Full(Some(42)))
        ));

        let err = Error::<i32>::TrySendError(ErrorType::Closed(Some(99)));
        assert!(err.is_closed());
        assert!(!err.is_full());

        let err = Error::<i32>::SendError(ErrorType::Closed(Some(99)));
        assert!(err.is_closed());

        let err = Error::<i32>::SendTimeoutError(ErrorType::Closed(Some(99)));
        assert!(err.is_closed());

        let err = Error::<i32>::SendTimeoutError(ErrorType::Timeout(Some(77)));
        assert!(err.is_timeout());
        assert!(!err.is_full());
        assert!(!err.is_closed());

        let err = Error::<i32>::RecvResultError;
        assert!(!err.is_full());
        assert!(!err.is_closed());
        assert!(!err.is_timeout());
    }

    #[test]
    fn test_error_type_debug_and_display() {
        let err = Error::<String>::TrySendError(ErrorType::Full(Some("hello".into())));
        assert!(!format!("{:?}", err).is_empty());
        assert!(!format!("{}", err).is_empty());
    }

    #[test]
    fn test_error_type_eq() {
        assert_eq!(
            ErrorType::<i32>::Full(Some(1)),
            ErrorType::<i32>::Full(Some(1))
        );
        assert_ne!(
            ErrorType::<i32>::Full(Some(1)),
            ErrorType::<i32>::Full(Some(2))
        );
        assert_ne!(
            ErrorType::<i32>::Full(Some(1)),
            ErrorType::<i32>::Closed(Some(1))
        );
    }

    #[test]
    fn test_error_none_values() {
        assert!(Error::<()>::SendError(ErrorType::Full(None)).is_full());
        assert!(Error::<()>::SendError(ErrorType::Closed(None)).is_closed());
        assert!(Error::<()>::SendTimeoutError(ErrorType::Timeout(None)).is_timeout());
    }

    // ------------------------------------------------------------------
    // 2. Builder tests (public API only - fields are private)
    // ------------------------------------------------------------------
    #[test]
    fn test_builder_build_default_queue_workers() {
        let (queue, _runner) = Builder::default().build();
        assert_eq!(queue.workers(), 100);
    }

    #[test]
    fn test_builder_custom_workers() {
        let (queue, _runner) = Builder::default().workers(42).build();
        assert_eq!(queue.workers(), 42);
    }

    #[test]
    fn test_builder_custom_queue_max_is_full_check() {
        let (queue, _runner) = Builder::default().queue_max(10).build();
        assert_eq!(queue.waiting_count(), 0);
    }

    #[test]
    fn test_builder_custom_both() {
        let (q, _r) = Builder::default().workers(8).queue_max(2048).build();
        assert_eq!(q.workers(), 8);
    }

    #[test]
    fn test_group_builder_build() {
        let (_queue, _runner): (TaskExecQueue<_, String, ()>, _) =
            Builder::default().group().build::<String>();
    }

    #[test]
    fn test_group_builder_default_workers() {
        let (queue, _runner): (TaskExecQueue<_, String, ()>, _) =
            Builder::default().group().build::<String>();
        assert_eq!(queue.workers(), 100);
    }

    #[test]
    fn test_builder_with_channel() {
        let (tx, rx) = futures::channel::mpsc::channel::<((), TaskType)>(10);
        let (_queue, _runner) = Builder::default().with_channel::<_, _, ()>(tx, rx).build();
    }

    #[test]
    fn test_builder_with_channel_group() {
        let (tx, rx) = futures::channel::mpsc::channel::<((), TaskType)>(10);
        let (_queue, _runner) = Builder::default()
            .with_channel::<_, _, ()>(tx, rx)
            .group()
            .build::<String>();
    }

    #[test]
    fn test_builder_build_default_types() {
        let (_queue, _runner): (TaskExecQueue, _) = Builder::default().build();
        let (_queue, _runner): (TaskExecQueue<_, (), ()>, _) = Builder::default().build();
    }

    #[test]
    fn test_group_channel_builder_combined() {
        let (tx, rx) = futures::channel::mpsc::channel::<((), TaskType)>(50);
        let (_queue, _runner) = Builder::default()
            .workers(3)
            .queue_max(50)
            .with_channel::<_, _, ()>(tx, rx)
            .group()
            .build::<String>();
    }

    // ------------------------------------------------------------------
    // 3. Queue state tests (non-async)
    // ------------------------------------------------------------------
    #[test]
    fn test_queue_state_initial() {
        let (queue, _runner) = Builder::default().workers(4).queue_max(1000).build();
        assert_eq!(queue.workers(), 4);
        assert_eq!(queue.active_count(), 0);
        assert_eq!(queue.waiting_count(), 0);
        assert!(!queue.is_active());
        assert!(!queue.is_closed());
        assert!(!queue.is_full());
        assert!(!queue.is_flushing());
        assert_eq!(queue.pending_wakers_count(), 0);
        assert_eq!(queue.waiting_wakers_count(), 0);
    }

    #[test]
    fn test_queue_state_is_full_initially_false() {
        let (queue, _runner) = Builder::default().queue_max(1).build();
        assert!(!queue.is_full());
    }

    #[test]
    fn test_queue_state_is_closed_initially_false() {
        let (queue, _runner) = Builder::default().build();
        assert!(!queue.is_closed());
    }

    // ------------------------------------------------------------------
    // 4. Spawner basic tests (async, multi-threaded tokio)
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_spawn_task_executes() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = queue
            .spawn(async move { f.store(true, Ordering::SeqCst) })
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_try_spawn_task_executes() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = queue
            .try_spawn(async move { f.store(true, Ordering::SeqCst) })
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_spawn_multiple_tasks() {
        let (queue, runner) = Builder::default().workers(5).queue_max(100).build();
        tokio::spawn(runner);

        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..10 {
            let c = counter.clone();
            let result = queue
                .spawn(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                })
                .await;
            assert!(result.is_ok());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_spawn_with_name() {
        // Custom channel with D = &'static str
        let (tx, rx) = futures::channel::mpsc::channel::<(&'static str, TaskType)>(100);
        let (queue, runner) = Builder::default()
            .workers(2)
            .queue_max(100)
            .with_channel::<_, _, &'static str>(tx, rx)
            .build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = queue
            .spawn_with(async move { f.store(true, Ordering::SeqCst) }, "named")
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_try_spawn_with_name() {
        let (tx, rx) = futures::channel::mpsc::channel::<(&'static str, TaskType)>(100);
        let (queue, runner) = Builder::default()
            .workers(2)
            .queue_max(100)
            .with_channel::<_, _, &'static str>(tx, rx)
            .build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = queue
            .try_spawn_with(async move { f.store(true, Ordering::SeqCst) }, "try_named")
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_spawn_result_returns_ok() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);
        let result = queue.spawn(async {}).await;
        assert!(result.is_ok());
    }

    // ------------------------------------------------------------------
    // 5. Group spawner tests (async) — uses GroupBuilder for G=String
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_group_same_key_sequential_order() {
        let (queue, runner): (TaskExecQueue<_, String, ()>, _) = Builder::default()
            .workers(2)
            .queue_max(100)
            .group()
            .build::<String>();
        tokio::spawn(runner);

        let results = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let r = results.clone();
        let result = queue
            .spawn(async move { r.lock().push(1) })
            .group("group_a".to_string())
            .await;
        assert!(result.is_ok());

        let r = results.clone();
        let result = queue
            .spawn(async move { r.lock().push(2) })
            .group("group_a".to_string())
            .await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(*results.lock(), vec![1, 2]);
    }

    #[tokio::test]
    async fn test_group_different_keys_concurrent() {
        let (queue, runner): (TaskExecQueue<_, String, ()>, _) = Builder::default()
            .workers(5)
            .queue_max(100)
            .group()
            .build::<String>();
        tokio::spawn(runner);

        let results = Arc::new(parking_lot::Mutex::new(Vec::new()));

        let r = results.clone();
        let result = queue
            .spawn(async move { r.lock().push("a1") })
            .group("grp_a".to_string())
            .await;
        assert!(result.is_ok());

        let r = results.clone();
        let result = queue
            .spawn(async move { r.lock().push("b1") })
            .group("grp_b".to_string())
            .await;
        assert!(result.is_ok());

        tokio::time::sleep(Duration::from_millis(200)).await;
        let vec = results.lock();
        assert_eq!(vec.len(), 2);
        assert!(vec.contains(&"a1"));
        assert!(vec.contains(&"b1"));
    }

    #[tokio::test]
    async fn test_try_group_spawner() {
        let (queue, runner): (TaskExecQueue<_, String, ()>, _) = Builder::default()
            .workers(2)
            .queue_max(100)
            .group()
            .build::<String>();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = queue
            .try_spawn(async move { f.store(true, Ordering::SeqCst) })
            .group("grp_x".to_string())
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_group_spawner_consecutive() {
        let (queue, runner): (TaskExecQueue<_, String, ()>, _) = Builder::default()
            .workers(2)
            .queue_max(100)
            .group()
            .build::<String>();
        tokio::spawn(runner);

        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let c = counter.clone();
            let result = queue
                .spawn(async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                })
                .group("seq".to_string())
                .await;
            assert!(result.is_ok());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    // ------------------------------------------------------------------
    // 6. Close / Flush tests (async)
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_close_drains_and_closes() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let _ = queue
            .spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                c.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        let result = queue.close().await;
        assert!(result.is_ok());
        assert!(queue.is_closed());
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_flush_waiting_tasks() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let c = counter.clone();
            let _ = queue
                .spawn(async move {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    c.fetch_add(1, Ordering::SeqCst);
                })
                .await;
        }

        let result = queue.flush().await;
        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_spawn_after_close_returns_error() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);
        let _ = queue.close().await;

        let result = queue.spawn(async {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_try_spawn_after_close_fails() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);
        let _ = queue.close().await;

        let result = queue.try_spawn(async {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_close_before_start() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        let runner_handle = tokio::spawn(runner);

        let result = queue.close().await;
        assert!(result.is_ok());
        assert!(queue.is_closed());
        let _ = tokio::time::timeout(Duration::from_secs(1), runner_handle).await;
    }

    // ------------------------------------------------------------------
    // 7. Local variant tests (single-threaded, uses LocalSet + spawn_local)
    // ------------------------------------------------------------------
    fn run_local<F, T>(f: F) -> T
    where
        F: Future<Output = T>,
    {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();
        rt.block_on(local.run_until(f))
    }

    #[test]
    fn test_local_spawn_task_executes() {
        run_local(async {
            let (queue, runner) = LocalBuilder::default().workers(2).queue_max(100).build();
            tokio::task::spawn_local(runner);

            let flag = Arc::new(AtomicBool::new(false));
            let f = flag.clone();
            let result = queue
                .spawn(async move { f.store(true, Ordering::SeqCst) })
                .await;
            assert!(result.is_ok());
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(flag.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn test_local_try_spawn() {
        run_local(async {
            let (queue, runner) = LocalBuilder::default().workers(2).queue_max(100).build();
            tokio::task::spawn_local(runner);

            let flag = Arc::new(AtomicBool::new(false));
            let f = flag.clone();
            let result = queue
                .try_spawn(async move { f.store(true, Ordering::SeqCst) })
                .await;
            assert!(result.is_ok());
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(flag.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn test_local_group_spawner() {
        run_local(async {
            let (queue, runner): (LocalTaskExecQueue<_, String, ()>, _) = LocalBuilder::default()
                .workers(2)
                .queue_max(100)
                .group()
                .build::<String>();
            tokio::task::spawn_local(runner);

            let results = Arc::new(parking_lot::Mutex::new(Vec::new()));
            let r = results.clone();
            let result = queue
                .spawn(async move { r.lock().push(1) })
                .group("g".to_string())
                .await;
            assert!(result.is_ok());

            let r = results.clone();
            let result = queue
                .spawn(async move { r.lock().push(2) })
                .group("g".to_string())
                .await;
            assert!(result.is_ok());

            tokio::time::sleep(Duration::from_millis(200)).await;
            assert_eq!(*results.lock(), vec![1, 2]);
        });
    }

    #[test]
    fn test_local_queue_state() {
        let (queue, _runner) = LocalBuilder::default().workers(3).queue_max(500).build();
        assert_eq!(queue.workers(), 3);
        assert!(!queue.is_closed());
        assert!(!queue.is_full());
        assert_eq!(queue.active_count(), 0);
        assert_eq!(queue.waiting_count(), 0);
    }

    #[test]
    fn test_local_close() {
        run_local(async {
            let (queue, runner) = LocalBuilder::default().workers(2).queue_max(100).build();
            tokio::task::spawn_local(runner);

            let counter = Arc::new(AtomicUsize::new(0));
            let c = counter.clone();
            let _ = queue
                .spawn(async move { c.fetch_add(1, Ordering::SeqCst) })
                .await;

            let result = queue.close().await;
            assert!(result.is_ok());
            assert!(queue.is_closed());
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn test_local_flush() {
        run_local(async {
            let (queue, runner) = LocalBuilder::default().workers(2).queue_max(100).build();
            tokio::task::spawn_local(runner);

            let counter = Arc::new(AtomicUsize::new(0));
            for _ in 0..3 {
                let c = counter.clone();
                let _ = queue
                    .spawn(async move { c.fetch_add(1, Ordering::SeqCst) })
                    .await;
            }

            let result = queue.flush().await;
            assert!(result.is_ok());
            assert_eq!(counter.load(Ordering::SeqCst), 3);
        });
    }

    #[test]
    fn test_local_spawn_with_name() {
        run_local(async {
            let (tx, rx) = futures::channel::mpsc::channel::<(&'static str, LocalTaskType)>(100);
            let (queue, runner) = LocalBuilder::default()
                .workers(2)
                .queue_max(100)
                .with_channel::<_, _, &'static str>(tx, rx)
                .build();
            tokio::task::spawn_local(runner);

            let flag = Arc::new(AtomicBool::new(false));
            let f = flag.clone();
            let result = queue
                .spawn_with(
                    async move { f.store(true, Ordering::SeqCst) },
                    "local_named",
                )
                .await;
            assert!(result.is_ok());
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(flag.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn test_local_try_group_spawner() {
        run_local(async {
            let (queue, runner): (LocalTaskExecQueue<_, String, ()>, _) = LocalBuilder::default()
                .workers(2)
                .queue_max(100)
                .group()
                .build::<String>();
            tokio::task::spawn_local(runner);

            let flag = Arc::new(AtomicBool::new(false));
            let f = flag.clone();
            let result = queue
                .try_spawn(async move { f.store(true, Ordering::SeqCst) })
                .group("g".to_string())
                .await;
            assert!(result.is_ok());
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert!(flag.load(Ordering::SeqCst));
        });
    }

    // ------------------------------------------------------------------
    // 8. Default spawner tests
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_spawn_default_with_manual_set() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        // set_default returns Result<(), TaskExecQueue> — handle without unwrap
        if set_default(queue).is_err() {
            // already set by another test — skip
            return;
        }

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = default()
            .spawn(async move { f.store(true, Ordering::SeqCst) })
            .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_default_already_set() {
        let (q1, r1) = Builder::default().workers(1).queue_max(10).build();
        let (q2, _r2) = Builder::default().workers(1).queue_max(10).build();
        tokio::spawn(r1);

        assert!(set_default(q1).is_ok());
        assert!(set_default(q2).is_err());
    }

    // ------------------------------------------------------------------
    // 9. SpawnExt trait tests (qualified calls to avoid ambiguity)
    // ------------------------------------------------------------------
    #[tokio::test]
    async fn test_spawn_ext_trait() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = SpawnExt::spawn(async move { f.store(true, Ordering::SeqCst) }, &queue).await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_spawn_ext_with_name() {
        let (tx, rx) = futures::channel::mpsc::channel::<(&'static str, TaskType)>(100);
        let (queue, runner) = Builder::default()
            .workers(2)
            .queue_max(100)
            .with_channel::<_, _, &'static str>(tx, rx)
            .build();
        tokio::spawn(runner);

        let flag = Arc::new(AtomicBool::new(false));
        let f = flag.clone();
        let result = SpawnExt::spawn_with(
            async move { f.store(true, Ordering::SeqCst) },
            &queue,
            "named",
        )
        .await;
        assert!(result.is_ok());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(flag.load(Ordering::SeqCst));
    }

    // ------------------------------------------------------------------
    // 10. Counter tests (internal helper)
    // ------------------------------------------------------------------
    #[test]
    fn test_counter_inc_dec_value() {
        let c = Counter::new();
        assert_eq!(c.value(), 0);
        c.inc();
        assert_eq!(c.value(), 1);
        c.inc();
        assert_eq!(c.value(), 2);
        c.dec();
        assert_eq!(c.value(), 1);
        c.dec();
        assert_eq!(c.value(), 0);
    }

    // ------------------------------------------------------------------
    // 11. Rate feature tests (conditional)
    // ------------------------------------------------------------------
    #[cfg(feature = "rate")]
    #[tokio::test]
    async fn test_rate_completed_count() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        for _ in 0..5 {
            let _ = queue.spawn(async {}).await;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(queue.completed_count().await >= 0);
    }

    #[cfg(feature = "rate")]
    #[tokio::test]
    async fn test_rate_method() {
        let (queue, runner) = Builder::default().workers(2).queue_max(100).build();
        tokio::spawn(runner);

        for _ in 0..10 {
            let _ = queue.spawn(async {}).await;
        }

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(queue.rate().await >= 0.0);
    }

    // ------------------------------------------------------------------
    // 12. LocalBuilder tests (public API only)
    // ------------------------------------------------------------------
    #[test]
    fn test_local_builder_default_workers() {
        let (queue, _runner) = LocalBuilder::default().build();
        assert_eq!(queue.workers(), 100);
    }

    #[test]
    fn test_local_builder_custom_workers() {
        let (queue, _runner) = LocalBuilder::default().workers(4).queue_max(500).build();
        assert_eq!(queue.workers(), 4);
    }

    #[test]
    fn test_local_group_builder() {
        let (_queue, _runner): (LocalTaskExecQueue<_, String, ()>, _) =
            LocalBuilder::default().workers(2).group().build::<String>();
    }

    #[test]
    fn test_local_channel_builder() {
        let (tx, rx) = futures::channel::mpsc::channel::<((), LocalTaskType)>(10);
        let (_queue, _runner) = LocalBuilder::default()
            .with_channel::<_, _, ()>(tx, rx)
            .build();
    }

    #[test]
    fn test_local_channel_builder_group() {
        let (tx, rx) = futures::channel::mpsc::channel::<((), LocalTaskType)>(10);
        let (_queue, _runner): (LocalTaskExecQueue<_, String, ()>, _) = LocalBuilder::default()
            .with_channel::<_, _, ()>(tx, rx)
            .group()
            .build::<String>();
    }
}
