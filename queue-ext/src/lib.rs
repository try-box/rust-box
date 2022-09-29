use std::fmt;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

#[allow(unreachable_pub)]
pub use self::queue_sender::QueueSender;
#[allow(unreachable_pub)]
pub use self::queue_stream::QueueStream;

mod queue_sender;
mod queue_stream;

pub trait Waker {
    fn rx_wake(&self);
    fn tx_park(&self, w: std::task::Waker);
}

impl<T: ?Sized> QueueExt for T {}

pub trait QueueExt {
    #[inline]
    fn queue_stream<Item, F>(self, f: F) -> QueueStream<Self, Item, F>
        where
            Self: Sized + Unpin,
            F: Fn(Pin<&mut Self>, &mut Context<'_>) -> Poll<Option<Item>>,
    {
        assert_stream::<Item, _>(QueueStream::new(self, f))
    }

    #[inline]
    fn queue_sender<Item, F, R>(self, f: F) -> QueueSender<Self, Item, F, R>
        where
            Self: Sized + Waker,
            F: Fn(&mut Self, Action<Item>) -> Reply<R>,
    {
        QueueSender::new(self, f)
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    fn queue_channel<Item, F1, R, F2>(
        self,
        f1: F1,
        f2: F2,
    ) -> (
        QueueSender<QueueStream<Self, Item, F2>, Item, F1, R>,
        QueueStream<Self, Item, F2>,
    )
        where
            Self: Sized + Unpin + Clone,
            F1: Fn(&mut QueueStream<Self, Item, F2>, Action<Item>) -> Reply<R>,
            F2: Fn(Pin<&mut Self>, &mut Context<'_>) -> Poll<Option<Item>> + Clone + Unpin,
    {
        queue_channel(self, f1, f2)
    }
}

#[allow(clippy::type_complexity)]
#[inline]
pub fn queue_channel<Q, Item, F1, R, F2>(
    q: Q,
    f1: F1,
    f2: F2,
) -> (
    QueueSender<QueueStream<Q, Item, F2>, Item, F1, R>,
    QueueStream<Q, Item, F2>,
)
    where
        Q: Sized + Unpin + Clone,
        F1: Fn(&mut QueueStream<Q, Item, F2>, Action<Item>) -> Reply<R>,
        F2: Fn(Pin<&mut Q>, &mut Context<'_>) -> Poll<Option<Item>> + Clone + Unpin,
{
    let rx = QueueStream::new(q, f2);
    let tx = QueueSender::new(rx.clone(), f1);
    (tx, rx)
}

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

pub type TrySendError<T> = SendError<T>;

#[derive(Clone, PartialEq, Eq)]
pub struct SendError<T> {
    kind: SendErrorKind,
    val: T,
}

impl<T> SendError<T> {
    #[inline]
    pub fn full(val: T) -> Self {
        SendError {
            kind: SendErrorKind::Full,
            val,
        }
    }

    #[inline]
    pub fn disconnected(val: T) -> Self {
        SendError {
            kind: SendErrorKind::Disconnected,
            val,
        }
    }
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
    #[inline]
    pub fn is_full(&self) -> bool {
        matches!(self.kind, SendErrorKind::Full)
    }

    /// Returns `true` if this error is a result of the receiver being dropped.
    #[inline]
    pub fn is_disconnected(&self) -> bool {
        matches!(self.kind, SendErrorKind::Disconnected)
    }

    /// Returns the message that was attempted to be sent but failed.
    #[inline]
    pub fn into_inner(self) -> T {
        self.val
    }
}

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
#[inline]
pub(crate) fn assert_stream<T, S>(stream: S) -> S
    where
        S: Stream<Item=T>,
{
    stream
}
