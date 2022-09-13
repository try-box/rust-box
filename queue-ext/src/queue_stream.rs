use std::fmt;
use std::marker::PhantomData;
use std::marker::Unpin;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

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
        _item: PhantomData<Item>,
    }
}

impl<Q, Item, F> Clone for QueueStream<Q, Item, F>
    where
        Q: Clone,
        F: Clone,
{
    fn clone(&self) -> Self {
        Self {
            q: self.q.clone(),
            f: self.f.clone(),
            recv_task: self.recv_task.clone(),
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
    pub(super) fn new(q: Q, f: F) -> Self {
        Self {
            q,
            f,
            recv_task: Arc::new(AtomicWaker::new()),
            _item: PhantomData,
        }
    }
}

impl<Q, Item, F> Waker for QueueStream<Q, Item, F> {
    fn wake(&self) {
        self.recv_task.wake()
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
            Poll::Ready(msg) => Poll::Ready(msg),
            Poll::Pending => {
                this.recv_task.register(ctx.waker());
                f(this.q.as_mut(), ctx)
            }
        }
    }
}

impl<Q, Item, F> Deref for QueueStream<Q, Item, F> {
    type Target = Q;
    fn deref(&self) -> &Self::Target {
        &self.q
    }
}

impl<Q, Item, F> DerefMut for QueueStream<Q, Item, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.q
    }
}
