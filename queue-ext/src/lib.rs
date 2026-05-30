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
    fn close_channel(&self);
    fn is_closed(&self) -> bool;
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
    Len,
}

pub enum Reply<R> {
    Send(R),
    IsFull(bool),
    IsEmpty(bool),
    Len(usize),
}

pub type TrySendError<T> = SendError<T>;

#[derive(Clone, PartialEq, Eq)]
pub struct SendError<T> {
    kind: SendErrorKind,
    val: Option<T>,
}

impl<T> SendError<T> {
    #[inline]
    pub fn full(val: T) -> Self {
        SendError {
            kind: SendErrorKind::Full,
            val: Some(val),
        }
    }

    #[inline]
    pub fn disconnected(val: Option<T>) -> Self {
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
            write!(f, "send failed because mpsc is full")
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
    /// Returns `true` if this error is a result of the mpsc being full.
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
    pub fn into_inner(self) -> Option<T> {
        self.val
    }
}

// Just a helper function to ensure the streams we're returning all have the
// right implementations.
#[inline]
pub(crate) fn assert_stream<T, S>(stream: S) -> S
where
    S: Stream<Item = T>,
{
    stream
}

#[cfg(test)]
use futures::pin_mut;
#[cfg(test)]
use futures::task::noop_waker;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// VecDeque-based helpers (no sharing — works when Q is not cloned)
// ---------------------------------------------------------------------------

#[cfg(test)]
fn poll_deque(pin_q: Pin<&mut VecDeque<i32>>, _cx: &mut Context<'_>) -> Poll<Option<i32>> {
    Poll::Ready(pin_q.get_mut().pop_front())
}

// ---------------------------------------------------------------------------
// SharedQueue — Arc-backed queue that shares data on Clone
// Required by queue_channel() which clones the underlying Q.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[derive(Clone)]
struct SharedQueue(Arc<Mutex<VecDeque<i32>>>);

#[cfg(test)]
type SharedPollFn = fn(Pin<&mut SharedQueue>, &mut Context<'_>) -> Poll<Option<i32>>;

#[cfg(test)]
type SharedHandlerFn =
    fn(&mut QueueStream<SharedQueue, i32, SharedPollFn>, Action<i32>) -> Reply<i32>;

#[cfg(test)]
fn poll_shared(pin_q: Pin<&mut SharedQueue>, _cx: &mut Context<'_>) -> Poll<Option<i32>> {
    Poll::Ready(pin_q.get_mut().0.lock().unwrap().pop_front())
}

#[cfg(test)]
fn push_shared(
    s: &mut QueueStream<SharedQueue, i32, SharedPollFn>,
    action: Action<i32>,
) -> Reply<i32> {
    match action {
        Action::Send(item) => {
            s.0.lock().unwrap().push_back(item);
            Reply::Send(item)
        }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.0.lock().unwrap().is_empty()),
        Action::Len => Reply::Len(s.0.lock().unwrap().len()),
    }
}

#[cfg(test)]
fn make_shared_channel() -> (
    QueueSender<QueueStream<SharedQueue, i32, SharedPollFn>, i32, SharedHandlerFn, i32>,
    QueueStream<SharedQueue, i32, SharedPollFn>,
) {
    let q = SharedQueue(Arc::new(Mutex::new(VecDeque::new())));
    queue_channel(
        q,
        push_shared as SharedHandlerFn,
        poll_shared as SharedPollFn,
    )
}

// ---------------------------------------------------------------------------
// queue_channel end-to-end
// ---------------------------------------------------------------------------

#[test]
fn queue_channel_send_receive() {
    let (mut tx, rx) = make_shared_channel();

    tx.try_send(1).unwrap();
    tx.try_send(2).unwrap();
    tx.try_send(3).unwrap();

    pin_mut!(rx);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(1)));
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(2)));
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(3)));
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

#[test]
fn queue_channel_sender_is_full() {
    let (mut tx, _rx) = make_shared_channel();
    assert!(!tx.is_full());
    tx.try_send(1).unwrap();
    assert_eq!(tx.len(), 1);
}

#[test]
fn queue_channel_receiver_closed_when_sender_dropped() {
    let (tx, rx) = make_shared_channel();
    assert!(!rx.is_closed());
    drop(tx);
    assert!(rx.is_closed());

    pin_mut!(rx);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

// ---------------------------------------------------------------------------
// QueueExt trait
// ---------------------------------------------------------------------------

#[test]
fn queue_ext_queue_stream() {
    let q: VecDeque<i32> = VecDeque::from([5, 10]);
    let stream = q.queue_stream(
        poll_deque as fn(Pin<&mut VecDeque<i32>>, &mut Context<'_>) -> Poll<Option<i32>>,
    );
    pin_mut!(stream);

    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(5)));
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(10)));
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

#[test]
fn queue_ext_queue_sender() {
    let stream_obj: QueueStream<SharedQueue, i32, SharedPollFn> = QueueStream::new(
        SharedQueue(Arc::new(Mutex::new(VecDeque::new()))),
        poll_shared as SharedPollFn,
    );
    let mut sender = stream_obj.queue_sender(push_shared as SharedHandlerFn);
    let r = sender.try_send(7);
    assert!(r.is_ok());
    assert_eq!(r.unwrap(), 7);
}

#[test]
fn queue_ext_queue_channel() {
    use futures::StreamExt;

    let q = SharedQueue(Arc::new(Mutex::new(VecDeque::new())));
    let (mut tx, rx) = q.queue_channel(push_shared as SharedHandlerFn, poll_shared as SharedPollFn);
    tx.try_send(100).unwrap();

    let items: Vec<i32> = futures::executor::block_on(rx.collect::<Vec<i32>>());
    assert_eq!(items, vec![100]);
}

// ---------------------------------------------------------------------------
// Multiple senders
// ---------------------------------------------------------------------------

#[test]
fn multiple_senders() {
    let (mut tx1, rx) = make_shared_channel();
    let mut tx2 = tx1.clone();
    let mut tx3 = tx1.clone();

    tx1.try_send(1).unwrap();
    tx2.try_send(2).unwrap();
    tx3.try_send(3).unwrap();

    assert_eq!(tx1.len(), 3);

    pin_mut!(rx);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(1)));
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(2)));
    assert_eq!(rx.as_mut().poll_next(&mut cx), Poll::Ready(Some(3)));
}

#[test]
fn channel_closes_only_when_last_sender_dropped() {
    let (tx1, rx) = make_shared_channel();
    let tx2 = tx1.clone();
    let tx3 = tx1.clone();

    assert!(!rx.is_closed());
    drop(tx1);
    assert!(!rx.is_closed(), "still have tx2, tx3");
    drop(tx2);
    assert!(!rx.is_closed(), "still have tx3");
    drop(tx3);
    assert!(rx.is_closed(), "all senders gone");
}

// ---------------------------------------------------------------------------
// Waker trait impl on QueueStream
// ---------------------------------------------------------------------------

#[test]
fn queue_stream_waker_rx_wake() {
    let stream: QueueStream<
        VecDeque<i32>,
        i32,
        fn(Pin<&mut VecDeque<i32>>, &mut Context<'_>) -> Poll<Option<i32>>,
    > = QueueStream::new(VecDeque::new(), poll_deque);
    stream.rx_wake();
}

#[test]
fn queue_stream_waker_is_closed() {
    let stream: QueueStream<
        VecDeque<i32>,
        i32,
        fn(Pin<&mut VecDeque<i32>>, &mut Context<'_>) -> Poll<Option<i32>>,
    > = QueueStream::new(VecDeque::new(), poll_deque);
    assert!(!Waker::is_closed(&stream));
    stream.close_channel();
    assert!(Waker::is_closed(&stream));
}
