use std::fmt;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Sink;
use pin_project_lite::pin_project;

use super::Waker;

pub enum Action<Item> {
    Send(Item),
    IsFull,
    IsEmpty,
}

pub enum Reply<R> {
    Send(R),
    IsFull(bool),
    IsEmpty(bool),
}

pin_project! {
    pub struct Sender<S, Item, F, R> {
        #[pin]
        s: S,
        #[pin]
        f: F,
        _item: PhantomData<Item>,
        _r: PhantomData<R>,
    }
}

impl<S, Item, F, R> Clone for Sender<S, Item, F, R>
    where
        S: Clone,
        F: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            s: self.s.clone(),
            f: self.f.clone(),
            _item: PhantomData,
            _r: PhantomData,
        }
    }
}

impl<S, Item, F, R> fmt::Debug for Sender<S, Item, F, R>
    where
        S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").field("stream", &self.s).finish()
    }
}

impl<S, Item, F, R> Sender<S, Item, F, R>
    where
        S: Waker,
        F: Fn(&mut S, Action<Item>) -> Reply<R>,
{
    #[inline]
    pub(super) fn new(s: S, f: F) -> Self {
        Self {
            s,
            f,
            _item: PhantomData,
            _r: PhantomData,
        }
    }

    #[inline]
    pub fn try_send(&mut self, item: Item) -> Result<R, TrySendError<Item>> {
        if self.is_full() {
            return Err(TrySendError {
                kind: SendErrorKind::Full,
                val: item,
            });
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
    fn is_full(&mut self) -> bool {
        match (self.f)(&mut self.s, Action::IsFull) {
            Reply::IsFull(true) => true,
            Reply::IsFull(false) => false,
            _ => unreachable!(),
        }
    }
}

impl<S, Item, F, R> Sink<Item> for Sender<S, Item, F, R>
    where
        S: Waker + Unpin,
        F: Fn(&mut S, Action<Item>) -> Reply<R>,
{
    type Error = SendError<Item>;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
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
        let mut this = self.project();
        let _ = (this.f)(&mut this.s, Action::Send(item));
        this.s.rx_wake();
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        println!("Sender poll_close ...");
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

impl<S, Item, F, R> std::convert::AsMut<S> for Sender<S, Item, F, R> {
    #[inline]
    fn as_mut(&mut self) -> &mut S {
        &mut self.s
    }
}

impl<S, Item, F, R> std::convert::AsRef<S> for Sender<S, Item, F, R> {
    #[inline]
    fn as_ref(&self) -> &S {
        &self.s
    }
}

pub type TrySendError<T> = SendError<T>;

#[derive(Clone, PartialEq, Eq)]
pub struct SendError<T> {
    kind: SendErrorKind,
    val: T,
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_full() {
            write!(f, "send failed because channel is full")
        } else {
            write!(f, "send failed because receiver is gone")
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendErrorKind {
    Full,
    Disconnected,
}

impl<T: core::any::Any> std::error::Error for SendError<T> {}

impl<T> SendError<T> {
    /// Returns `true` if this error is a result of the channel being full.
    pub fn is_full(&self) -> bool {
        match self.kind {
            SendErrorKind::Full => true,
            _ => false,
        }
    }

    /// Returns `true` if this error is a result of the receiver being dropped.
    pub fn is_disconnected(&self) -> bool {
        match self.kind {
            SendErrorKind::Disconnected => true,
            _ => false,
        }
    }

    /// Returns the message that was attempted to be sent but failed.
    pub fn into_inner(self) -> T {
        self.val
    }
}
