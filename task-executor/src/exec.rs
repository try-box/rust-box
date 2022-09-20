use std::collections::HashSet;
use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use futures::{ready, Sink, SinkExt, StreamExt};
use futures::channel::mpsc;
use futures::task::AtomicWaker;
use parking_lot::RwLock;
#[cfg(feature = "rate")]
use update_rate::{DiscreteRateCounter, RateCounter};

use queue_ext::{Action, QueueExt, Reply};

use super::{assert_future, Counter, Error, ErrorType, Spawner};

type TaskType = Box<dyn std::future::Future<Output=()> + Send + 'static + Unpin>;

pub struct Executor {
    pub(crate) tx: mpsc::Sender<TaskType>,
    workers: usize,
    queue_max: isize,
    active_count: Counter,
    pub(crate) waiting_count: Counter,
    completed_count: Counter,
    #[cfg(feature = "rate")]
    rate_counter: Arc<RwLock<DiscreteRateCounter>>,
    close_waker: Arc<AtomicWaker>,
    is_closing: Arc<AtomicBool>,
}

impl Clone for Executor {
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
            close_waker: self.close_waker.clone(),
            is_closing: self.is_closing.clone(),
        }
    }
}

impl Executor {
    #[inline]
    pub(crate) fn new(workers: usize, queue_max: usize) -> (Self, impl Future<Output=()>) {
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
            close_waker: Arc::new(AtomicWaker::new()),
            is_closing: Arc::new(AtomicBool::new(false)),
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
        if self.is_closed() || self.is_closing() {
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
    pub fn spawn<T>(&mut self, msg: T) -> Spawner<'_, T>
        where
            T: Future + Send + 'static,
            T::Output: Send + 'static,
    {
        let fut = Spawner::new(self, msg);
        assert_future::<Result<(), _>, _>(fut)
    }

    #[inline]
    pub fn close(&mut self) -> Close<'_, mpsc::Sender<TaskType>> {
        self.is_closing.store(true, Ordering::SeqCst);
        Close::new(
            &mut self.tx,
            self.waiting_count.clone(),
            self.active_count.clone(),
            self.close_waker.clone(),
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
    pub fn is_closing(&self) -> bool {
        self.is_closing.load(Ordering::SeqCst)
    }

    async fn run(self, mut task_rx: mpsc::Receiver<TaskType>) {
        let exec = self;
        let idle_waker = Arc::new(AtomicWaker::new());

        let channel = || {
            let rx = OneValue::new().queue_stream(|s, _| match s.take() {
                None => Poll::Pending,
                Some(m) => Poll::Ready(Some(m)),
            });

            let tx = rx.clone().sender(|s, action| match action {
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

                    if exec.is_closing() && rx.is_empty() {
                        exec.close_waker.wake();
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
}

#[derive(Clone)]
struct IndexSet(Arc<RwLock<HashSet<usize>>>);

impl IndexSet {
    #[inline]
    fn new() -> Self {
        Self(Arc::new(RwLock::new(HashSet::new())))
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

pub struct Close<'a, Si: ?Sized> {
    sink: &'a mut Si,
    waiting_count: Counter,
    active_count: Counter,
    w: Arc<AtomicWaker>,
}

impl<Si: Unpin + ?Sized> Unpin for Close<'_, Si> {}

impl<'a, Si: Sink<TaskType> + Unpin + ?Sized> Close<'a, Si> {
    fn new(
        sink: &'a mut Si,
        waiting_count: Counter,
        active_count: Counter,
        w: Arc<AtomicWaker>,
    ) -> Self {
        Self {
            sink,
            waiting_count,
            active_count,
            w,
        }
    }
}

impl<Si: Sink<TaskType> + Unpin + ?Sized> Future for Close<'_, Si> {
    type Output = Result<(), Si::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        ready!(Pin::new(&mut self.sink).poll_close(cx))?;
        if self.waiting_count.value() > 0 || self.active_count.value() > 0 {
            self.w.register(cx.waker());
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
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
