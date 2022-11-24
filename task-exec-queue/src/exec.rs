use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::marker::Unpin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

use futures::{Sink, SinkExt, Stream, StreamExt};
use futures::channel::mpsc;
use futures::task::AtomicWaker;
use parking_lot::Mutex;
use parking_lot::RwLock;
#[cfg(feature = "rate")]
use update_rate::{DiscreteRateCounter, RateCounter};

use queue_ext::{Action, QueueExt, Reply};

use super::{
    assert_future, close::Close, Counter, Error, ErrorType, flush::Flush, GroupTaskExecQueue,
    IndexSet, PendingOnce, Spawner, TrySpawner,
};

type DashMap<K, V> = dashmap::DashMap<K, V, ahash::RandomState>;
type GroupChannels<G> = Arc<DashMap<G, Arc<Mutex<GroupTaskExecQueue<TaskType>>>>>;

pub type TaskType = Box<dyn std::future::Future<Output=()> + Send + 'static + Unpin>;

pub struct TaskExecQueue<Tx = mpsc::Sender<((), TaskType)>, G = (), D = ()> {
    pub(crate) tx: Tx,
    workers: usize,
    queue_max: isize,
    active_count: Counter,
    pub(crate) waiting_count: Counter,
    completed_count: Counter,
    #[cfg(feature = "rate")]
    rate_counter: Arc<RwLock<DiscreteRateCounter>>,
    flush_waker: Arc<AtomicWaker>,
    is_flushing: Arc<AtomicBool>,
    is_closed: Arc<AtomicBool>,

    //group
    group_channels: GroupChannels<G>,
    _d: std::marker::PhantomData<D>,
}

impl<Tx, G, D> Clone for TaskExecQueue<Tx, G, D>
    where
        Tx: Clone,
{
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
            is_closed: self.is_closed.clone(),
            group_channels: self.group_channels.clone(),
            _d: std::marker::PhantomData,
        }
    }
}

impl<Tx, G, D> TaskExecQueue<Tx, G, D>
    where
        Tx: Clone + Sink<(D, TaskType)> + Unpin + Send + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub(crate) fn with_channel<Rx>(
        workers: usize,
        queue_max: usize,
        tx: Tx,
        rx: Rx,
    ) -> (Self, impl Future<Output=()>)
        where
            Rx: Stream<Item=(D, TaskType)> + Unpin,
    {
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
            is_closed: Arc::new(AtomicBool::new(false)),
            group_channels: Arc::new(DashMap::default()),
            _d: std::marker::PhantomData,
        };
        let runner = exec.clone().run(rx);
        (exec, runner)
    }

    #[inline]
    pub fn try_spawn_with<T>(&self, msg: T, name: D) -> TrySpawner<'_, T, Tx, G, D>
        where
            D: Clone,
            T: Future + Send + 'static,
            T::Output: Send + 'static,
    {
        let fut = TrySpawner::new(self, msg, name);
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub fn spawn_with<T>(&self, msg: T, name: D) -> Spawner<'_, T, Tx, G, D>
        where
            D: Clone,
            T: Future + Send + 'static,
            T::Output: Send + 'static,
    {
        let fut = Spawner::new(self, msg, name);
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub fn flush(&self) -> Flush<Tx, D> {
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
    pub fn close(&self) -> Close<Tx, D> {
        self.is_flushing.store(true, Ordering::SeqCst);
        self.is_closed.store(true, Ordering::SeqCst);
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
        self.is_closed.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn is_flushing(&self) -> bool {
        self.is_flushing.load(Ordering::SeqCst)
    }

    async fn run<Rx>(self, mut task_rx: Rx)
        where
            Rx: Stream<Item=(D, TaskType)> + Unpin,
    {
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
                Action::Len => Reply::Len(s.len()),
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
            while let Some((_, task)) = task_rx.next().await {
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
        log::info!("exit task execution queue");
    }
}

impl<Tx, G> TaskExecQueue<Tx, G, ()>
    where
        Tx: Clone + Sink<((), TaskType)> + Unpin + Send + Sync + 'static,
        G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    #[inline]
    pub fn try_spawn<T>(&self, task: T) -> TrySpawner<'_, T, Tx, G, ()>
        where
            T: Future + Send + 'static,
            T::Output: Send + 'static,
    {
        let fut = TrySpawner::new(self, task, ());
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub fn spawn<T>(&self, task: T) -> Spawner<'_, T, Tx, G, ()>
        where
            T: Future + Send + 'static,
            T::Output: Send + 'static,
    {
        let fut = Spawner::new(self, task, ());
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub(crate) async fn group_send(&self, name: G, task: TaskType) -> Result<(), Error<TaskType>> {
        if self.is_closed() {
            return Err(Error::SendError(ErrorType::Closed(Some(task))));
        }

        let gt_queue = self
            .group_channels
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Mutex::new(GroupTaskExecQueue::new())))
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
            if (self
                .tx
                .clone()
                .send(((), Box::new(Box::pin(runner_task))))
                .await)
                .is_err()
            {
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

    #[inline]
    fn len(&self) -> usize {
        if self.0.read().is_some() {
            1
        }else{
            0
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.0.read().is_none()
    }
}
