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

#[cfg(test)]
use futures::task::noop_waker;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::Mutex;

/// Test helper: a minimal Waker implementation backed by a VecDeque.
#[cfg(test)]
#[derive(Clone)]
struct TestStream {
    queue: Arc<Mutex<VecDeque<i32>>>,
    closed: Arc<AtomicBool>,
}

#[cfg(test)]
impl TestStream {
    fn new() -> Self {
        TestStream {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
impl Waker for TestStream {
    fn rx_wake(&self) {}
    fn tx_park(&self, _w: std::task::Waker) {}
    fn close_channel(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

/// Small-capacity handler: is_full when len >= 3.
#[cfg(test)]
fn bounded_handler(s: &mut TestStream, action: Action<i32>) -> Reply<i32> {
    match action {
        Action::Send(item) => {
            s.queue.lock().unwrap().push_back(item);
            Reply::Send(item)
        }
        Action::IsFull => Reply::IsFull(s.queue.lock().unwrap().len() >= 3),
        Action::IsEmpty => Reply::IsEmpty(s.queue.lock().unwrap().is_empty()),
        Action::Len => Reply::Len(s.queue.lock().unwrap().len()),
    }
}

/// Unbounded handler: never full.
#[cfg(test)]
fn unbounded_handler(s: &mut TestStream, action: Action<i32>) -> Reply<i32> {
    match action {
        Action::Send(item) => {
            s.queue.lock().unwrap().push_back(item);
            Reply::Send(item)
        }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.queue.lock().unwrap().is_empty()),
        Action::Len => Reply::Len(s.queue.lock().unwrap().len()),
    }
}

// ---------------------------------------------------------------------------
// QueueSender::try_send tests
// ---------------------------------------------------------------------------

#[test]
fn try_send_ok() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);
    let r = sender.try_send(42);
    assert!(r.is_ok());
    assert_eq!(r.unwrap(), 42);
}

#[test]
fn try_send_err_full() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);
    sender.try_send(1).unwrap();
    sender.try_send(2).unwrap();
    sender.try_send(3).unwrap();

    let err = sender.try_send(4).unwrap_err();
    assert!(err.is_full());
    assert!(!err.is_disconnected());
    assert_eq!(err.into_inner(), Some(4));
}

#[test]
fn try_send_err_disconnected() {
    let s = TestStream::new();
    s.closed.store(true, Ordering::SeqCst);
    let mut sender = QueueSender::new(s, unbounded_handler);
    let err = sender.try_send(42).unwrap_err();
    assert!(err.is_disconnected());
    assert!(!err.is_full());
    assert_eq!(err.into_inner(), Some(42));
}

// ---------------------------------------------------------------------------
// is_full / is_empty / len
// ---------------------------------------------------------------------------

#[test]
fn state_methods() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);

    assert!(sender.is_empty());
    assert!(!sender.is_full());
    assert_eq!(sender.len(), 0);

    sender.try_send(1).unwrap();
    assert!(!sender.is_empty());
    assert!(!sender.is_full());
    assert_eq!(sender.len(), 1);

    sender.try_send(2).unwrap();
    sender.try_send(3).unwrap();
    assert!(!sender.is_empty());
    assert!(sender.is_full());
    assert_eq!(sender.len(), 3);
}

// ---------------------------------------------------------------------------
// SendError
// ---------------------------------------------------------------------------

#[test]
fn send_error_full_ctor() {
    let err = SendError::full(42i32);
    assert!(err.is_full());
    assert!(!err.is_disconnected());
    assert_eq!(err.into_inner(), Some(42));
}

#[test]
fn send_error_disconnected_ctor() {
    let err = SendError::<i32>::disconnected(Some(99));
    assert!(err.is_disconnected());
    assert!(!err.is_full());
    assert_eq!(err.into_inner(), Some(99));
}

#[test]
fn send_error_disconnected_none() {
    let err = SendError::<i32>::disconnected(None);
    assert!(err.is_disconnected());
    assert_eq!(err.into_inner(), None);
}

#[test]
fn send_error_debug() {
    let err = SendError::full(42i32);
    let s = format!("{:?}", err);
    assert!(s.contains("SendError"));
}

#[test]
fn send_error_display_full() {
    let err = SendError::full(42i32);
    assert_eq!(format!("{}", err), "send failed because mpsc is full");
}

#[test]
fn send_error_display_disconnected() {
    let err = SendError::<i32>::disconnected(None);
    assert_eq!(format!("{}", err), "send failed because receiver is gone");
}

#[test]
fn send_error_clone_eq() {
    let err1 = SendError::full(42i32);
    let err2 = err1.clone();
    assert_eq!(err1, err2);
    assert!(err1.is_full());
    assert_eq!(err2.into_inner(), Some(42));
}

// ---------------------------------------------------------------------------
// Sink impl
// ---------------------------------------------------------------------------

#[test]
fn sink_poll_ready_ok() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(
        Pin::new(&mut sender).poll_ready(&mut cx),
        Poll::Ready(Ok(()))
    );
}

#[test]
fn sink_poll_ready_closed() {
    let s = TestStream::new();
    s.closed.store(true, Ordering::SeqCst);
    let mut sender = QueueSender::new(s, unbounded_handler);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let r = Pin::new(&mut sender).poll_ready(&mut cx);
    assert!(matches!(r, Poll::Ready(Err(ref e)) if e.is_disconnected()));
}

#[test]
fn sink_start_send_ok() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);
    assert!(Pin::new(&mut sender).start_send(42).is_ok());
}

#[test]
fn sink_start_send_closed() {
    let s = TestStream::new();
    s.closed.store(true, Ordering::SeqCst);
    let mut sender = QueueSender::new(s, unbounded_handler);
    let r = Pin::new(&mut sender).start_send(42);
    assert!(r.is_err());
    assert!(r.unwrap_err().is_disconnected());
}

#[test]
fn sink_poll_flush_ok() {
    let mut sender = QueueSender::new(TestStream::new(), bounded_handler);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(
        Pin::new(&mut sender).poll_flush(&mut cx),
        Poll::Ready(Ok(()))
    );
}

#[test]
fn sink_poll_flush_closed() {
    let s = TestStream::new();
    s.closed.store(true, Ordering::SeqCst);
    let mut sender = QueueSender::new(s, unbounded_handler);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let r = Pin::new(&mut sender).poll_flush(&mut cx);
    assert!(matches!(r, Poll::Ready(Err(ref e)) if e.is_disconnected()));
}

#[test]
fn sink_poll_closes_single_sender() {
    let mut sender = QueueSender::new(TestStream::new(), unbounded_handler);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    // Single sender, queue empty → poll_close returns Ready(Ok(()))
    assert_eq!(
        Pin::new(&mut sender).poll_close(&mut cx),
        Poll::Ready(Ok(()))
    );
}

// ---------------------------------------------------------------------------
// Drop behaviour
// ---------------------------------------------------------------------------

#[test]
fn drop_last_sender_closes_channel() {
    let s = TestStream::new();
    let closed = s.closed.clone();
    let sender = QueueSender::new(s, unbounded_handler);
    drop(sender);
    assert!(closed.load(Ordering::SeqCst));
}

#[test]
fn drop_clone_does_not_close_immediately() {
    let s = TestStream::new();
    let closed = s.closed.clone();
    let sender1 = QueueSender::new(s, unbounded_handler);
    let sender2 = sender1.clone();

    drop(sender2);
    // num_senders: 2→1, prev == 2, so close_channel NOT called
    assert!(!closed.load(Ordering::SeqCst));

    drop(sender1);
    // num_senders: 1→0, prev == 1, so close_channel IS called
    assert!(closed.load(Ordering::SeqCst));
}

// ---------------------------------------------------------------------------
// Send / Sync compile check
// ---------------------------------------------------------------------------

#[test]
fn sender_is_send_sync() {
    fn assert_send<T: Send>(_t: &T) {}
    fn assert_sync<T: Sync>(_t: &T) {}

    let s = TestStream::new();
    let sender = QueueSender::new(s, unbounded_handler);
    assert_send(&sender);
    assert_sync(&sender);
}
