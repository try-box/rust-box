use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::channel::mpsc;
use futures::task::AtomicWaker;
use futures::{ready, Sink, SinkExt, StreamExt};
use parking_lot::{Mutex, RwLock};
#[cfg(feature = "rate")]
use update_rate::{DiscreteRateCounter, RateCounter};

use queue_ext::{Action, QueueExt, Reply};

use super::{assert_future, Counter, Error, ErrorType, Spawner};

type DashMap<K, V> = dashmap::DashMap<K, V, ahash::RandomState>;
type TaskType = Box<dyn std::future::Future<Output = ()> + Send + 'static + Unpin>;
type GroupChannels<G> = Arc<DashMap<G, Arc<Mutex<GroupTaskVecDeque>>>>;

pub struct Executor<G = ()> {
    pub(crate) tx: mpsc::Sender<TaskType>,
    workers: usize,
    queue_max: isize,
    active_count: Counter,
    pub(crate) waiting_count: Counter,
    completed_count: Counter,
    #[cfg(feature = "rate")]
    rate_counter: Arc<RwLock<DiscreteRateCounter>>,
    flush_waker: Arc<AtomicWaker>,
    is_flushing: Arc<AtomicBool>,

    //group
    group_channels: GroupChannels<G>,
}

impl<G> Clone for Executor<G> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            workers: self.workers,
            queue_max: self.queue_max,
            active_count: self.active_count.clone(),
            waiting_count: self.waiting_count.clone(),
            completed_count: self.completed_count.clone(),
            #[cfg(feature = "rate")]
            rate_counter: self.rate_counter.clone(),
            flush_waker: self.flush_waker.clone(),
            is_flushing: self.is_flushing.clone(),
            group_channels: self.group_channels.clone(),
        }
    }
}

impl<G> Executor<G>
where
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn new(workers: usize, queue_max: usize) -> (Self, impl Future<Output = ()>) {
        let (tx, rx) = mpsc::channel(queue_max);
        let exec = Self {
            tx,
            workers,
            queue_max: queue_max as isize,
            active_count: Counter::new(),
            waiting_count: Counter::new(),
            completed_count: Counter::new(),
            #[cfg(feature = "rate")]
            rate_counter: Arc::new(RwLock::new(DiscreteRateCounter::new(100))),
            flush_waker: Arc::new(AtomicWaker::new()),
            is_flushing: Arc::new(AtomicBool::new(false)),
            group_channels: Arc::new(DashMap::default()),
        };
        let runner = exec.clone().run(rx);
        (exec, runner)
    }

    #[inline]
    pub fn try_spawn<T>(&self, task: T) -> Result<(), Error<T>>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        if self.is_closed() {
            return Err(Error::TrySendError(ErrorType::Closed(Some(task))));
        }
        if self.is_full() {
            return Err(Error::TrySendError(ErrorType::Full(Some(task))));
        }
        let waiting_count = self.waiting_count.clone();
        let task = async move {
            waiting_count.dec();
            let _ = task.await;
        };
        self.waiting_count.inc();

        if let Err(_e) = self.tx.clone().try_send(Box::new(Box::pin(task))) {
            self.waiting_count.dec();
            Err(Error::TrySendError(ErrorType::Closed(None)))
        } else {
            Ok(())
        }
    }

    #[inline]
    pub fn spawn<T>(&mut self, msg: T) -> Spawner<'_, T, G>
    where
        T: Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let fut = Spawner::new(self, msg);
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub fn flush(&self) -> Flush {
        self.is_flushing.store(true, Ordering::SeqCst);
        Flush::new(
            self.tx.clone(),
            self.waiting_count.clone(),
            self.active_count.clone(),
            self.is_flushing.clone(),
            self.flush_waker.clone(),
        )
    }

    #[inline]
    pub fn close(&self) -> Close {
        self.is_flushing.store(true, Ordering::SeqCst);
        Close::new(
            self.tx.clone(),
            self.waiting_count.clone(),
            self.active_count.clone(),
            self.is_flushing.clone(),
            self.flush_waker.clone(),
        )
    }

    #[inline]
    pub fn workers(&self) -> usize {
        self.workers
    }

    #[inline]
    pub fn active_count(&self) -> isize {
        self.active_count.value()
    }

    #[inline]
    pub fn waiting_count(&self) -> isize {
        self.waiting_count.value()
    }

    #[inline]
    pub fn completed_count(&self) -> isize {
        self.completed_count.value()
    }

    #[inline]
    #[cfg(feature = "rate")]
    pub fn rate(&self) -> f64 {
        self.rate_counter.read().rate()
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.waiting_count() >= self.queue_max
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    #[inline]
    pub fn is_flushing(&self) -> bool {
        self.is_flushing.load(Ordering::SeqCst)
    }

    async fn run(self, mut task_rx: mpsc::Receiver<TaskType>) {
        let exec = self;
        let idle_waker = Arc::new(AtomicWaker::new());

        let channel = || {
            let rx = OneValue::new().queue_stream(|s, _| match s.take() {
                None => Poll::Pending,
                Some(m) => Poll::Ready(Some(m)),
            });

            let tx = rx.clone().queue_sender(|s, action| match action {
                Action::Send(item) => Reply::Send(s.set(item)),
                Action::IsFull => Reply::IsFull(s.is_full()),
                Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
            });

            (tx, rx)
        };

        let idle_idxs = IndexSet::new();
        let mut txs = Vec::new();
        let mut rxs = Vec::new();
        for i in 0..exec.workers {
            let (tx, mut rx) = channel();
            let idle_waker = idle_waker.clone();
            let idle_idxs = idle_idxs.clone();
            idle_idxs.insert(i);
            let exec = exec.clone();
            let rx_fut = async move {
                loop {
                    match rx.next().await {
                        Some(task) => {
                            exec.active_count.inc();
                            task.await;
                            exec.completed_count.inc();
                            exec.active_count.dec();
                            #[cfg(feature = "rate")]
                            exec.rate_counter.write().update();
                        }
                        None => break,
                    }

                    if !rx.is_full() {
                        idle_idxs.insert(i);
                        idle_waker.wake();
                    }

                    if exec.is_flushing() && rx.is_empty() {
                        exec.flush_waker.wake();
                    }
                }
            };

            txs.push(tx);
            rxs.push(rx_fut);
        }

        let tasks_bus = async move {
            while let Some(task) = task_rx.next().await {
                loop {
                    if idle_idxs.is_empty() {
                        //sleep ...
                        PendingOnce::new(idle_waker.clone()).await;
                    } else if let Some(idx) = idle_idxs.pop() {
                        //select ...
                        if let Some(tx) = txs.get_mut(idx) {
                            if let Err(_t) = tx.send(task).await {
                                log::error!("send error ...");
                                // task = t.into_inner();
                            }
                        }
                        break;
                    };
                }
            }
        };

        futures::future::join(tasks_bus, futures::future::join_all(rxs)).await;
        log::info!("exit task executor");
    }

    #[inline]
    pub(crate) async fn group_send(&self, name: G, task: TaskType) -> Result<(), Error<TaskType>> {
        if self.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(Some(task))));
        }

        let gt_queue = self
            .group_channels
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Mutex::new(GroupTaskVecDeque::new())))
            .value()
            .clone();

        let exec = self.clone();
        let group_channels = self.group_channels.clone();
        let runner_task = {
            let mut task_tx = gt_queue.lock();
            if task_tx.is_running() {
                task_tx.push(task);
                drop(task_tx);
                drop(gt_queue);
                None
            } else {
                task_tx.set_running(true);
                drop(task_tx);
                let task_rx = gt_queue; //.clone();
                let runner_task = async move {
                    exec.active_count.inc();
                    task.await;
                    exec.active_count.dec();
                    loop {
                        let task: Option<TaskType> = task_rx.lock().pop();
                        if let Some(task) = task {
                            exec.active_count.inc();
                            task.await;
                            exec.completed_count.inc();
                            exec.active_count.dec();
                        } else {
                            group_channels.remove(&name);
                            break;
                        }
                    }
                };
                Some(runner_task)
            }
        };

        if let Some(runner_task) = runner_task {
            if (self.tx.clone().send(Box::new(Box::pin(runner_task))).await).is_err() {
                Err(Error::SendError(ErrorType::Closed(None)))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
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

#[derive(Clone)]
struct OneValue(Arc<RwLock<Option<TaskType>>>);

unsafe impl Sync for OneValue {}

unsafe impl Send for OneValue {}

impl OneValue {
    #[inline]
    fn new() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }

    #[inline]
    fn set(&self, val: TaskType) -> Option<TaskType> {
        self.0.write().replace(val)
    }

    #[inline]
    fn take(&self) -> Option<TaskType> {
        self.0.write().take()
    }

    #[inline]
    fn is_full(&self) -> bool {
        self.0.read().is_some()
    }

    fn is_empty(&self) -> bool {
        self.0.read().is_none()
    }
}

struct PendingOnce {
    w: Arc<AtomicWaker>,
    is_ready: bool,
}

impl PendingOnce {
    #[inline]
    fn new(w: Arc<AtomicWaker>) -> Self {
        Self { w, is_ready: false }
    }
}

impl Future for PendingOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_ready {
            Poll::Ready(())
        } else {
            self.w.register(cx.waker());
            self.is_ready = true;
            Poll::Pending
        }
    }
}

pub struct Flush {
    sink: mpsc::Sender<TaskType>,
    waiting_count: Counter,
    active_count: Counter,
    is_flushing: Arc<AtomicBool>,
    w: Arc<AtomicWaker>,
}

impl Unpin for Flush {}

impl Flush {
    fn new(
        sink: mpsc::Sender<TaskType>,
        waiting_count: Counter,
        active_count: Counter,
        is_flushing: Arc<AtomicBool>,
        w: Arc<AtomicWaker>,
    ) -> Self {
        Self {
            sink,
            waiting_count,
            active_count,
            is_flushing,
            w,
        }
    }
}

impl Future for Flush {
    type Output = Result<(), mpsc::SendError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.sink).poll_flush(cx))?;
        if self.waiting_count.value() > 0 || self.active_count.value() > 0 {
            self.w.register(cx.waker());
            Poll::Pending
        } else {
            self.is_flushing.store(false, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
}

pub struct Close {
    sink: mpsc::Sender<TaskType>,
    waiting_count: Counter,
    active_count: Counter,
    is_flushing: Arc<AtomicBool>,
    w: Arc<AtomicWaker>,
}

impl Unpin for Close {}

impl Close {
    fn new(
        sink: mpsc::Sender<TaskType>,
        waiting_count: Counter,
        active_count: Counter,
        is_flushing: Arc<AtomicBool>,
        w: Arc<AtomicWaker>,
    ) -> Self {
        Self {
            sink,
            waiting_count,
            active_count,
            is_flushing,
            w,
        }
    }
}

impl Future for Close {
    type Output = Result<(), mpsc::SendError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.sink).poll_close(cx))?;
        if self.waiting_count.value() > 0 || self.active_count.value() > 0 {
            self.w.register(cx.waker());
            Poll::Pending
        } else {
            self.is_flushing.store(false, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
}

struct GroupTaskVecDeque {
    tasks: VecDeque<TaskType>,
    is_running: bool,
}

impl GroupTaskVecDeque {
    #[inline]
    fn new() -> Self {
        Self {
            tasks: VecDeque::default(),
            is_running: false,
        }
    }

    #[inline]
    fn push(&mut self, task: TaskType) {
        self.tasks.push_back(task);
    }

    #[inline]
    fn pop(&mut self) -> Option<TaskType> {
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
