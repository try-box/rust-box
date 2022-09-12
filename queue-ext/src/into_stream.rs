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
    #[derive(Clone)]
    #[must_use = "streams do nothing unless polled"]
    pub struct IntoStream<S, Item, F> {
        #[pin]
        s: S,
        #[pin]
        f: F,
        recv_task: Arc<AtomicWaker>,
        _item: PhantomData<Item>,
    }
}

impl<S, Item, F> fmt::Debug for IntoStream<S, Item, F>
    where
        S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoStream")
            .field("stream", &self.s)
            .finish()
    }
}

impl<S: Unpin, Item, F> IntoStream<S, Item, F> {
    pub(super) fn new(s: S, f: F) -> Self {
        Self {
            s,
            f,
            recv_task: Arc::new(AtomicWaker::new()),
            _item: PhantomData,
        }
    }
}

impl<S, Item, F> Waker for IntoStream<S, Item, F> {
    fn wake(&self) {
        self.recv_task.wake()
    }
}

impl<S, Item, F> Stream for IntoStream<S, Item, F>
    where
        S: Unpin,
        F: Fn(Pin<&mut S>, &mut Context<'_>) -> Poll<Option<Item>>,
{
    type Item = Item;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        let f = this.f.as_mut();
        match f(this.s.as_mut(), ctx) {
            Poll::Ready(msg) => Poll::Ready(msg),
            Poll::Pending => {
                this.recv_task.register(ctx.waker());
                f(this.s.as_mut(), ctx)
            }
        }
    }
}

impl<S, Item, F> Deref for IntoStream<S, Item, F> {
    type Target = S;
    fn deref(&self) -> &Self::Target {
        &self.s
    }
}

impl<S, Item, F> DerefMut for IntoStream<S, Item, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.s
    }
}
