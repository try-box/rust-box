use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::marker::Unpin;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures::task::AtomicWaker;
use futures::Stream;
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
        if !self.closed.load(Ordering::SeqCst) {
            self.closed.store(true, Ordering::SeqCst);
            self.rx_wake();
            if let Some(w) = self.parked_queue.lock().unwrap().pop_front() {
                w.wake();
            }
        }
    }

    #[inline]
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
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
                } else {
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

#[cfg(test)]
use futures::pin_mut;
#[cfg(test)]
use futures::task::noop_waker;
#[cfg(test)]
use std::cell::Cell;

/// Minimal queue type for testing QueueStream.
#[cfg(test)]
struct TestQueue {
    items: VecDeque<i32>,
}

#[cfg(test)]
fn poll_items(pin_q: Pin<&mut TestQueue>, _cx: &mut Context<'_>) -> Poll<Option<i32>> {
    Poll::Ready(pin_q.get_mut().items.pop_front())
}

#[cfg(test)]
fn poll_pending(_pin_q: Pin<&mut TestQueue>, _cx: &mut Context<'_>) -> Poll<Option<i32>> {
    Poll::Pending
}

// ---------------------------------------------------------------------------
// poll_next
// ---------------------------------------------------------------------------

#[test]
fn poll_next_yields_items() {
    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::from([10, 20, 30]),
        },
        poll_items,
    );
    pin_mut!(stream);

    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(10)));
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(20)));
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(Some(30)));
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

#[test]
fn poll_next_none_when_empty() {
    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_items,
    );
    pin_mut!(stream);

    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

// ---------------------------------------------------------------------------
// is_closed
// ---------------------------------------------------------------------------

#[test]
fn is_closed_open() {
    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );
    assert!(!stream.is_closed());
}

#[test]
fn is_closed_after_close() {
    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );
    stream.close_channel();
    assert!(stream.is_closed());
}

// ---------------------------------------------------------------------------
// Stream termination when channel is closed
// ---------------------------------------------------------------------------

#[test]
fn poll_next_terminates_on_closed() {
    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );
    stream.close_channel();

    pin_mut!(stream);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

#[test]
fn close_channel_during_pending_returns_none() {
    let call_count = Cell::new(0u32);
    let poll_fn = |_: Pin<&mut TestQueue>, _cx: &mut Context<'_>| -> Poll<Option<i32>> {
        call_count.set(call_count.get() + 1);
        Poll::Pending
    };

    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_fn,
    );

    pin_mut!(stream);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    // First poll: Pending, waker registered
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Pending);

    // Close the channel while stream is waiting (simulating sender drop)
    stream.close_channel();

    // Second poll: should now return Ready(None) because closed flag is set
    assert_eq!(stream.as_mut().poll_next(&mut cx), Poll::Ready(None));
}

// ---------------------------------------------------------------------------
// Waker registration on pending
// ---------------------------------------------------------------------------

#[test]
fn waker_registered_on_pending() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let woken = Arc::new(AtomicBool::new(false));
    let woken_clone = woken.clone();

    struct TestWaker(Arc<AtomicBool>);
    impl futures::task::ArcWake for TestWaker {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.0.store(true, Ordering::SeqCst);
        }
    }

    let test_waker = Arc::new(TestWaker(woken_clone));
    let waker = futures::task::waker(test_waker);

    let poll_fn =
        |_: Pin<&mut TestQueue>, _cx: &mut Context<'_>| -> Poll<Option<i32>> { Poll::Pending };

    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_fn,
    );
    pin_mut!(stream);

    let mut cx = Context::from_waker(&waker);
    let _ = stream.as_mut().poll_next(&mut cx);

    // Simulate a sender calling rx_wake()
    stream.rx_wake();

    // After wake, the waker flag should be set
    assert!(woken.load(Ordering::SeqCst));
}

// ---------------------------------------------------------------------------
// Waker trait impl on QueueStream
// ---------------------------------------------------------------------------

#[test]
fn queue_stream_implements_waker() {
    fn requires_waker<T: super::Waker>(_t: &T) {}

    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );
    requires_waker(&stream);
}

#[test]
fn queue_stream_waker_tx_park_and_wake() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );

    let woken = Arc::new(AtomicBool::new(false));
    let woken_clone = woken.clone();

    struct TestWaker(Arc<AtomicBool>);
    impl futures::task::ArcWake for TestWaker {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.0.store(true, Ordering::SeqCst);
        }
    }

    let waker = futures::task::waker(Arc::new(TestWaker(woken_clone)));

    // Park a waker
    stream.tx_park(waker);

    // close_channel should pop the parked waker and wake it
    assert!(!woken.load(Ordering::SeqCst));
    stream.close_channel();
    assert!(woken.load(Ordering::SeqCst));
}

// ---------------------------------------------------------------------------
// Send / Sync compile check
// ---------------------------------------------------------------------------

#[test]
fn queue_stream_is_send_sync() {
    fn assert_send<T: Send>(_t: &T) {}
    fn assert_sync<T: Sync>(_t: &T) {}

    let stream: QueueStream<TestQueue, i32, _> = QueueStream::new(
        TestQueue {
            items: VecDeque::new(),
        },
        poll_pending,
    );
    assert_send(&stream);
    assert_sync(&stream);
}
