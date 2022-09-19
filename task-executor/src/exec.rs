use std::collections::BTreeSet;
use std::fmt::Debug;
use std::future::Future;
use std::marker::PhantomData;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use crossbeam_queue::ArrayQueue;
use futures::{ready, Sink, SinkExt, StreamExt};
use futures::channel::mpsc;
use futures::task::AtomicWaker;
use parking_lot::RwLock;
use pin_project_lite::pin_project;
#[cfg(feature = "rate")]
use update_rate::{DiscreteRateCounter, RateCounter};

use queue_ext::{Action, QueueExt, Reply};

use super::Counter;

pub struct Executor<Item> {
    tx: mpsc::Sender<Item>,
    workers: usize,
    active_count: Counter,
    waiting_count: Counter,
    completed_count: Counter,
    #[cfg(feature = "rate")]
    rate_counter: Arc<RwLock<DiscreteRateCounter>>,
    close_waker: Arc<AtomicWaker>,
    is_closed: Arc<AtomicBool>,
}

impl<Item> Clone for Executor<Item> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            workers: self.workers,
            active_count: self.active_count.clone(),
            waiting_count: self.waiting_count.clone(),
            completed_count: self.completed_count.clone(),
            #[cfg(feature = "rate")]
            rate_counter: self.rate_counter.clone(),
            close_waker: self.close_waker.clone(),
            is_closed: self.is_closed.clone(),
        }
    }
}

impl<Item> Executor<Item>
    where
        Item: Future + Send + 'static,
{
    #[inline]
    pub(crate) fn new(workers: usize, queue_max: usize) -> (Self, impl Future<Output=()>) {
        let (tx, rx) = mpsc::channel(queue_max);
        let exec = Self {
            tx,
            workers,
            active_count: Counter::new(),
            waiting_count: Counter::new(),
            completed_count: Counter::new(),
            #[cfg(feature = "rate")]
            rate_counter: Arc::new(RwLock::new(DiscreteRateCounter::new(100))),
            close_waker: Arc::new(AtomicWaker::new()),
            is_closed: Arc::new(AtomicBool::new(false)),
        };
        let runner = exec.clone().run(rx);
        (exec, runner)
    }

    #[inline]
    pub fn mailbox(&self) -> Mailbox<mpsc::Sender<Item>, Item> {
        Mailbox::new(self.tx.clone(), self.waiting_count.clone())
    }

    #[inline]
    pub fn close(&mut self) -> Close<'_, mpsc::Sender<Item>, Item> {
        self.is_closed.store(true, Ordering::SeqCst);
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

    async fn run(self, mut task_rx: mpsc::Receiver<Item>)
        where
            Item: Future + Send,
    {
        let exec = self;
        let idle_waker = Arc::new(AtomicWaker::new());

        let channel = || {
            let rx = Arc::new(ArrayQueue::new(1)).queue_stream(|s, _| {
                if s.is_empty() {
                    Poll::Pending
                } else {
                    match s.pop() {
                        Some(m) => Poll::Ready(Some(m)),
                        None => Poll::Pending,
                    }
                }
            });

            let tx = rx.clone().sender(|s, action| match action {
                Action::Send(item) => Reply::Send(s.push(item)),
                Action::IsFull => Reply::IsFull(s.is_full()),
                Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
            });

            (tx, rx)
        };

        let idle_idxs = Arc::new(RwLock::new(BTreeSet::new()));
        let mut txs = Vec::new();
        let mut rxs = Vec::new();
        for i in 0..exec.workers {
            let (tx, mut rx) = channel();
            let idle_waker = idle_waker.clone();
            let idle_idxs = idle_idxs.clone();
            idle_idxs.write().insert(i);
            let exec = exec.clone();
            let rx_fut = async move {
                loop {
                    match rx.next().await {
                        Some(task) => {
                            exec.active_count.inc();
                            exec.waiting_count.dec();
                            task.await;
                            exec.completed_count.inc();
                            exec.active_count.dec();
                            #[cfg(feature = "rate")]
                            exec.rate_counter.write().update();
                        }
                        None => break,
                    }

                    if !rx.is_full() {
                        idle_idxs.write().insert(i);
                        idle_waker.wake();
                    }
                    if exec.is_closed.load(Ordering::SeqCst) && rx.is_empty() {
                        exec.close_waker.wake();
                    }
                }
            };

            txs.push(tx);
            rxs.push(rx_fut);
        }

        let tasks_bus = async move {
            while let Some(task) = task_rx.next().await {
                let mut _tx = None;
                loop {
                    if idle_idxs.read().is_empty() {
                        //sleep ...
                        PendingOnce::new(idle_waker.clone()).await;
                    } else {
                        //select ...
                        let mut idle_idxs_mut = idle_idxs.write();
                        let idx = if let Some(idx) = idle_idxs_mut.iter().next() {
                            *idx
                        } else {
                            unreachable!()
                        };
                        idle_idxs_mut.remove(&idx);
                        drop(idle_idxs_mut);
                        _tx = txs.get(idx).cloned();
                        break;
                    };
                }

                if let Err(_t) = _tx.unwrap().send(task).await {
                    log::error!("send error ...");
                    // task = t.into_inner();
                }
            }
        };

        futures::future::join(tasks_bus, futures::future::join_all(rxs)).await;
        log::info!("exit task executor");
    }
}

pin_project! {
    #[derive(Debug)]
    #[must_use = "sinks do nothing unless polled"]
    pub struct Mailbox<Si, Item> {
        #[pin]
        sink: Si,
        waiting_count: Counter,
        _item: std::marker::PhantomData<Item>,
    }
}

impl<Si, Item> Clone for Mailbox<Si, Item>
    where
        Si: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            waiting_count: self.waiting_count.clone(),
            _item: std::marker::PhantomData,
        }
    }
}

impl<Si: Sink<Item>, Item> Mailbox<Si, Item> {
    fn new(sink: Si, waiting_count: Counter) -> Self {
        Self {
            sink,
            waiting_count,
            _item: std::marker::PhantomData,
        }
    }
}

impl<Si, Item> Sink<Item> for Mailbox<Si, Item>
    where
        Si: Sink<Item>,
{
    type Error = Si::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        let this = self.project();
        this.sink.start_send(item)?;
        this.waiting_count.inc();
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().sink.poll_close(cx)
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

pub struct Close<'a, Si: ?Sized, Item> {
    sink: &'a mut Si,
    waiting_count: Counter,
    active_count: Counter,
    w: Arc<AtomicWaker>,
    _phantom: PhantomData<fn(Item)>,
}

impl<Si: Unpin + ?Sized, Item> Unpin for Close<'_, Si, Item> {}

impl<'a, Si: Sink<Item> + Unpin + ?Sized, Item> Close<'a, Si, Item> {
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
            _phantom: PhantomData,
        }
    }
}

impl<Si: Sink<Item> + Unpin + ?Sized, Item> Future for Close<'_, Si, Item> {
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
