use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Sink;
use pin_project::{pin_project, pinned_drop};

use super::{Action, Reply, SendError, TrySendError, Waker};

#[pin_project(PinnedDrop)]
pub struct QueueSender<S: Waker, Item, F, R> {
    #[pin]
    s: S,
    #[pin]
    f: F,
    num_senders: Arc<AtomicUsize>,
    _item: PhantomData<Item>,
    _r: PhantomData<R>,
}

unsafe impl<S: Waker, Item, F, R> Sync for QueueSender<S, Item, F, R> {}

unsafe impl<S: Waker, Item, F, R> Send for QueueSender<S, Item, F, R> {}

impl<S, Item, F, R> Clone for QueueSender<S, Item, F, R>
where
    S: Clone + Waker,
    F: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        //if !self.s.is_closed() {
        self.num_senders.fetch_add(1, Ordering::SeqCst);
        //}
        Self {
            s: self.s.clone(),
            f: self.f.clone(),
            num_senders: self.num_senders.clone(),
            _item: PhantomData,
            _r: PhantomData,
        }
    }
}

#[pinned_drop]
impl<S: Waker, Item, F, R> PinnedDrop for QueueSender<S, Item, F, R> {
    fn drop(self: Pin<&mut Self>) {
        self.set_closed();
    }
}

impl<S, Item, F, R> fmt::Debug for QueueSender<S, Item, F, R>
where
    S: fmt::Debug + Waker,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueSender")
            .field("stream", &self.s)
            .finish()
    }
}

impl<S, Item, F, R> QueueSender<S, Item, F, R>
where
    S: Waker,
{
    #[inline]
    fn set_closed(&self) -> usize {
        let prev = self.num_senders.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            self.s.close_channel();
        }
        prev
    }
}

impl<S, Item, F, R> QueueSender<S, Item, F, R>
where
    S: Waker,
    F: Fn(&mut S, Action<Item>) -> Reply<R>,
{
    #[inline]
    pub(super) fn new(s: S, f: F) -> Self {
        Self {
            s,
            f,
            num_senders: Arc::new(AtomicUsize::new(1)),
            _item: PhantomData,
            _r: PhantomData,
        }
    }

    #[inline]
    pub fn try_send(&mut self, item: Item) -> Result<R, TrySendError<Item>> {
        if self.s.is_closed() {
            return Err(SendError::disconnected(Some(item)));
        }
        if self.is_full() {
            return Err(TrySendError::full(item));
        }
        let res = (self.f)(&mut self.s, Action::Send(item));
        self.s.rx_wake();
        if let Reply::Send(r) = res {
            Ok(r)
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn is_full(&mut self) -> bool {
        match (self.f)(&mut self.s, Action::IsFull) {
            Reply::IsFull(reply) => reply,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn is_empty(&mut self) -> bool {
        match (self.f)(&mut self.s, Action::IsEmpty) {
            Reply::IsEmpty(reply) => reply,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn len(&mut self) -> usize {
        match (self.f)(&mut self.s, Action::Len) {
            Reply::Len(reply) => reply,
            _ => unreachable!(),
        }
    }
}

impl<S, Item, F, R> Sink<Item> for QueueSender<S, Item, F, R>
where
    S: Waker + Unpin,
    F: Fn(&mut S, Action<Item>) -> Reply<R>,
{
    type Error = SendError<Item>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.s.is_closed() {
            return Poll::Ready(Err(SendError::disconnected(None)));
        }
        let mut this = self.project();
        match (this.f)(&mut this.s, Action::IsFull) {
            Reply::IsFull(true) => {
                this.s.tx_park(cx.waker().clone());
                Poll::Pending
            }
            Reply::IsFull(false) => Poll::Ready(Ok(())),
            _ => unreachable!(),
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        if self.s.is_closed() {
            return Err(SendError::disconnected(Some(item)));
        }
        let mut this = self.project();
        let _ = (this.f)(&mut this.s, Action::Send(item));
        this.s.rx_wake();
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.s.is_closed() {
            return Poll::Ready(Err(SendError::disconnected(None)));
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.s.is_closed() {
            return Poll::Ready(Err(SendError::disconnected(None)));
        }
        if self.set_closed() > 1 {
            return Poll::Ready(Ok(()));
        }

        let mut this = self.project();
        match (this.f)(&mut this.s, Action::IsEmpty) {
            Reply::IsEmpty(true) => Poll::Ready(Ok(())),
            Reply::IsEmpty(false) => {
                this.s.tx_park(cx.waker().clone());
                Poll::Pending
            }
            _ => unreachable!(),
        }
    }
}

impl<S: Unpin + Waker, Item, F, R> std::convert::AsMut<S> for QueueSender<S, Item, F, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut S {
        &mut self.s
    }
}

impl<S: Waker, Item, F, R> std::convert::AsRef<S> for QueueSender<S, Item, F, R> {
    #[inline]
    fn as_ref(&self) -> &S {
        &self.s
    }
}
