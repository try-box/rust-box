use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::marker::Unpin;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::sync::atomic::{AtomicBool, Ordering};

use futures::Stream;
use futures::task::AtomicWaker;
use pin_project_lite::pin_project;

use super::Waker;

pin_project! {
    #[must_use = "streams do nothing unless polled"]
    pub struct QueueStream<Q, Item, F> {
        #[pin]
        q: Q,
        #[pin]
        f: F,
        recv_task: Arc<AtomicWaker>,
        parked_queue: Arc<Mutex<VecDeque<std::task::Waker>>>,
        closed: Arc<AtomicBool>,
        _item: PhantomData<Item>,
    }
}

unsafe impl<Q, Item, F> Sync for QueueStream<Q, Item, F> {}

unsafe impl<Q, Item, F> Send for QueueStream<Q, Item, F> {}

impl<Q, Item, F> Clone for QueueStream<Q, Item, F>
    where
        Q: Clone,
        F: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            q: self.q.clone(),
            f: self.f.clone(),
            recv_task: self.recv_task.clone(),
            parked_queue: self.parked_queue.clone(),
            closed: self.closed.clone(),
            _item: PhantomData,
        }
    }
}

impl<Q, Item, F> fmt::Debug for QueueStream<Q, Item, F>
    where
        Q: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueStream")
            .field("queue", &self.q)
            .finish()
    }
}

impl<Q: Unpin, Item, F> QueueStream<Q, Item, F> {
    #[inline]
    pub(super) fn new(q: Q, f: F) -> Self {
        Self {
            q,
            f,
            recv_task: Arc::new(AtomicWaker::new()),
            parked_queue: Arc::new(Mutex::new(VecDeque::default())),
            closed: Arc::new(AtomicBool::new(false)),
            _item: PhantomData,
        }
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

impl<Q, Item, F> Waker for QueueStream<Q, Item, F> {
    #[inline]
    fn rx_wake(&self) {
        self.recv_task.wake()
    }

    #[inline]
    fn tx_park(&self, w: std::task::Waker) {
        self.parked_queue.lock().unwrap().push_back(w);
    }

    #[inline]
    fn close_channel(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.rx_wake();
    }
}

impl<Q, Item, F> Stream for QueueStream<Q, Item, F>
    where
        Q: Unpin,
        F: Fn(Pin<&mut Q>, &mut Context<'_>) -> Poll<Option<Item>>,
{
    type Item = Item;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let f = this.f.as_mut();
        match f(this.q.as_mut(), ctx) {
            Poll::Ready(msg) => {
                if let Some(w) = this.parked_queue.lock().unwrap().pop_front() {
                    w.wake();
                }
                Poll::Ready(msg)
            }
            Poll::Pending => {
                if this.closed.load(Ordering::SeqCst) {
                    Poll::Ready(None)
                }else {
                    this.recv_task.register(ctx.waker());
                    f(this.q.as_mut(), ctx)
                }
            }
        }
    }
}

impl<Q, Item, F> Deref for QueueStream<Q, Item, F> {
    type Target = Q;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.q
    }
}

impl<Q, Item, F> DerefMut for QueueStream<Q, Item, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.q
    }
}
