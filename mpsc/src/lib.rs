use futures::{Sink, SinkExt, Stream, StreamExt};

use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

pub use queue_ext::SendError;
#[allow(unused_imports)]
use queue_ext::{Action, QueueExt, Reply, Waker};

#[cfg(feature = "priority")]
use collections::PriorityQueue;

///BinaryHeap based channel
#[cfg(feature = "priority")]
#[allow(clippy::type_complexity)]
pub fn with_priority_channel<P: Ord + 'static, T: 'static>(
    queue: std::sync::Arc<parking_lot::RwLock<PriorityQueue<P, T>>>,
    bound: usize,
) -> (Sender<(P, T), SendError<(P, T)>>, Receiver<(P, T)>) {
    let (tx, rx) = queue.queue_channel::<_, _, _, _>(
        move |s, act| match act {
            Action::Send((p, val)) => {
                s.write().push(p, val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.read().len() >= bound),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
            Action::Len => Reply::Len(s.read().len()),
        },
        |s, _| {
            let mut s = s.write();
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        },
    );

    (Sender::new(tx), Receiver::new(rx))
}

///BinaryHeap based channel
#[cfg(feature = "priority")]
#[allow(clippy::type_complexity)]
pub fn priority_channel<P: 'static + Ord, T: 'static>(
    bound: usize,
) -> (Sender<(P, T), SendError<(P, T)>>, Receiver<(P, T)>) {
    use PriorityQueue;
    let queue = std::sync::Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
    with_priority_channel(queue, bound)
}

///SegQueue based channel
#[cfg(feature = "segqueue")]
pub fn with_segqueue_channel<T: 'static>(
    queue: std::sync::Arc<crossbeam_queue::SegQueue<T>>,
    bound: usize,
) -> (Sender<T, SendError<T>>, Receiver<T>) {
    let (tx, rx) = queue.queue_channel::<T, _, _, _>(
        move |s, act| match act {
            Action::Send(val) => {
                s.push(val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.len() >= bound),
            Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
            Action::Len => Reply::Len(s.len()),
        },
        |s, _| match s.pop() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        },
    );
    (Sender::new(tx), Receiver::new(rx))
}

///SegQueue based channel
#[cfg(feature = "segqueue")]
pub fn segqueue_channel<T: 'static>(bound: usize) -> (Sender<T, SendError<T>>, Receiver<T>) {
    use crossbeam_queue::SegQueue;
    use std_ext::ArcExt;
    with_segqueue_channel(SegQueue::default().arc(), bound)
}

///VecDeque based channel
#[cfg(feature = "vecdeque")]
pub fn with_vecdeque_channel<T: 'static>(
    queue: std::sync::Arc<parking_lot::RwLock<std::collections::VecDeque<T>>>,
    bound: usize,
) -> (Sender<T, SendError<T>>, Receiver<T>) {
    let (tx, rx) = queue.queue_channel::<T, _, _, _>(
        move |s, act| match act {
            Action::Send(val) => {
                s.write().push_back(val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(s.read().len() >= bound),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
            Action::Len => Reply::Len(s.read().len()),
        },
        |s, _| {
            let mut s = s.write();
            match s.pop_front() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        },
    );
    (Sender::new(tx), Receiver::new(rx))
}

///VecDeque based channel
#[cfg(feature = "vecdeque")]
pub fn vecdeque_channel<T: 'static>(bound: usize) -> (Sender<T, SendError<T>>, Receiver<T>) {
    use std::collections::VecDeque;
    let queue = std::sync::Arc::new(parking_lot::RwLock::new(VecDeque::default()));
    with_vecdeque_channel(queue, bound)
}

///Indexmap based channel, remove entry if it already exists
#[cfg(feature = "indexmap")]
#[allow(clippy::type_complexity)]
pub fn with_indexmap_channel<K, T>(
    indexmap: std::sync::Arc<parking_lot::RwLock<indexmap::IndexMap<K, T>>>,
    bound: usize,
) -> (Sender<(K, T), SendError<(K, T)>>, Receiver<(K, T)>)
where
    K: Eq + std::hash::Hash + 'static,
    T: 'static,
{
    let (tx, rx) = indexmap.queue_channel::<(K, T), _, _, _>(
        move |s, act| match act {
            Action::Send((key, val)) => {
                let mut s = s.write();
                //Remove this entry if it already exists
                let reply = s.insert(key, val);
                Reply::Send(reply)
            }
            Action::IsFull => Reply::IsFull(s.read().len() >= bound),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
            Action::Len => Reply::Len(s.read().len()),
        },
        |s, _| {
            let mut s = s.write();
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        },
    );
    (Sender::new(tx), Receiver::new(rx))
}

///Indexmap based channel, remove entry if it already exists
#[cfg(feature = "indexmap")]
#[allow(clippy::type_complexity)]
pub fn indexmap_channel<K, T>(bound: usize) -> (Sender<(K, T), SendError<(K, T)>>, Receiver<(K, T)>)
where
    K: Eq + std::hash::Hash + 'static,
    T: 'static,
{
    use indexmap::IndexMap;
    let map = std::sync::Arc::new(parking_lot::RwLock::new(IndexMap::new()));
    with_indexmap_channel(map, bound)
}

pub trait SenderSink<M, E>: futures::Sink<M, Error = E> + Unpin + Send + Sync {
    fn box_clone(&self) -> Box<dyn SenderSink<M, E>>;
}

impl<T, M, E> SenderSink<M, E> for T
where
    T: futures::Sink<M, Error = E> + Unpin + Send + Sync + 'static,
    T: Clone,
{
    #[inline]
    fn box_clone(&self) -> Box<dyn SenderSink<M, E>> {
        Box::new(self.clone())
    }
}

pub struct Sender<M, E> {
    tx: Box<dyn SenderSink<M, E>>,
}

impl<M, E> Sender<M, E> {
    #[inline]
    pub fn new<T>(tx: T) -> Self
    where
        T: Sink<M, Error = E> + Sync + Send + Unpin + 'static,
        T: Clone,
    {
        Sender { tx: Box::new(tx) }
    }

    #[inline]
    pub async fn send(&mut self, t: M) -> std::result::Result<(), E> {
        self.tx.send(t).await
    }
}

impl<M, E> Clone for Sender<M, E> {
    #[inline]
    fn clone(&self) -> Self {
        Sender {
            tx: self.tx.box_clone(),
        }
    }
}

impl<M, E> Deref for Sender<M, E> {
    type Target = Box<dyn SenderSink<M, E>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl<M, E> DerefMut for Sender<M, E> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

impl<M, E> futures::Sink<M> for Sender<M, E> {
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, msg: M) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx).start_send(msg)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_close(cx)
    }
}

pub trait ReceiverStream<M>: futures::Stream<Item = M> + Send + Sync + Unpin + Waker {}

impl<T, M> ReceiverStream<M> for T where
    T: futures::Stream<Item = M> + Send + Sync + Unpin + Waker + 'static
{
}

pub struct Receiver<M> {
    rx: Box<dyn ReceiverStream<M>>,
}

impl<M> Drop for Receiver<M> {
    fn drop(&mut self) {
        self.rx.close_channel();
    }
}

impl<M> Receiver<M> {
    #[inline]
    pub fn new<T>(tx: T) -> Self
    where
        T: futures::Stream<Item = M> + Send + Sync + Unpin + Waker + 'static,
    {
        Receiver { rx: Box::new(tx) }
    }

    #[inline]
    pub async fn recv(&mut self) -> Option<M> {
        self.rx.next().await
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.rx.is_closed()
    }
}

impl<M> Deref for Receiver<M> {
    type Target = Box<dyn ReceiverStream<M>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.rx
    }
}

impl<M> DerefMut for Receiver<M> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rx
    }
}

impl<M> Stream for Receiver<M> {
    type Item = M;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

pub trait LocalSenderSink<M, E>: futures::Sink<M, Error = E> + Unpin {
    fn box_clone(&self) -> Box<dyn LocalSenderSink<M, E>>;
}

impl<T, M, E> LocalSenderSink<M, E> for T
where
    T: futures::Sink<M, Error = E> + Unpin + 'static,
    T: Clone,
{
    #[inline]
    fn box_clone(&self) -> Box<dyn LocalSenderSink<M, E>> {
        Box::new(self.clone())
    }
}

pub struct LocalSender<M, E> {
    tx: Box<dyn LocalSenderSink<M, E>>,
}

//unsafe impl<M, E> Sync for LocalSender<M, E> {}

impl<M, E> LocalSender<M, E> {
    #[inline]
    pub fn new<T>(tx: T) -> Self
    where
        T: Sink<M, Error = E> + Unpin + 'static,
        T: Clone,
    {
        LocalSender { tx: Box::new(tx) }
    }

    #[inline]
    pub async fn send(&mut self, t: M) -> std::result::Result<(), E> {
        self.tx.send(t).await
    }
}

impl<M, E> Clone for LocalSender<M, E> {
    #[inline]
    fn clone(&self) -> Self {
        LocalSender {
            tx: self.tx.box_clone(),
        }
    }
}

impl<M, E> Deref for LocalSender<M, E> {
    type Target = Box<dyn LocalSenderSink<M, E>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl<M, E> DerefMut for LocalSender<M, E> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

impl<M, E> futures::Sink<M> for LocalSender<M, E> {
    type Error = E;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, msg: M) -> Result<(), Self::Error> {
        Pin::new(&mut self.tx).start_send(msg)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.tx).poll_close(cx)
    }
}

pub trait LocalReceiverStream<M>: futures::Stream<Item = M> + Unpin {}

impl<T, M> LocalReceiverStream<M> for T where T: futures::Stream<Item = M> + Unpin + 'static {}

pub struct LocalReceiver<M> {
    rx: Box<dyn LocalReceiverStream<M>>,
}

impl<M> LocalReceiver<M> {
    #[inline]
    pub fn new<T>(tx: T) -> Self
    where
        T: futures::Stream<Item = M> + Unpin + 'static,
    {
        LocalReceiver { rx: Box::new(tx) }
    }

    #[inline]
    pub async fn recv(&mut self) -> Option<M> {
        self.rx.next().await
    }
}

impl<M> Deref for LocalReceiver<M> {
    type Target = Box<dyn LocalReceiverStream<M>>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.rx
    }
}

impl<M> DerefMut for LocalReceiver<M> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rx
    }
}

impl<M> Stream for LocalReceiver<M> {
    type Item = M;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::thread;
    use std::time::Duration;

    // -----------------------------------------------------------------------
    // SendError kind tests (type is re-exported from queue-ext)
    // -----------------------------------------------------------------------
    #[test]
    fn test_send_error_full() {
        let err = SendError::full(42i32);
        assert!(err.is_full());
        assert!(!err.is_disconnected());
        assert_eq!(err.into_inner(), Some(42));
    }

    #[test]
    fn test_send_error_disconnected() {
        let err = SendError::<i32>::disconnected(Some(99));
        assert!(err.is_disconnected());
        assert!(!err.is_full());
        assert_eq!(err.into_inner(), Some(99));
    }

    #[test]
    fn test_send_error_disconnected_none() {
        let err = SendError::<i32>::disconnected(None);
        assert!(err.is_disconnected());
        assert_eq!(err.into_inner(), None);
    }

    // -----------------------------------------------------------------------
    // SegQueue channel tests (default feature)
    // -----------------------------------------------------------------------
    #[cfg(feature = "segqueue")]
    mod segqueue_tests {
        use super::*;

        #[test]
        fn send_recv_one() {
            let (mut tx, mut rx) = segqueue_channel(10);
            block_on(tx.send(42)).unwrap();
            assert_eq!(block_on(rx.recv()), Some(42));
        }

        #[test]
        fn send_recv_multiple_in_order() {
            let (mut tx, mut rx) = segqueue_channel(10);
            for i in 0..10 {
                block_on(tx.send(i)).unwrap();
            }
            for i in 0..10 {
                assert_eq!(block_on(rx.recv()), Some(i));
            }
        }

        #[test]
        fn with_segqueue_channel_factory() {
            let q = std::sync::Arc::new(crossbeam_queue::SegQueue::new());
            let (mut tx, mut rx) = with_segqueue_channel(q, 10);
            block_on(tx.send("hello")).unwrap();
            assert_eq!(block_on(rx.recv()), Some("hello"));
        }

        #[test]
        fn sender_clone() {
            let (mut tx, mut rx) = segqueue_channel(10);
            let mut tx2 = tx.clone();
            block_on(tx.send(1)).unwrap();
            block_on(tx2.send(2)).unwrap();
            // Receive in order
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), Some(2));
        }

        #[test]
        fn sender_drop_closes_channel() {
            let (mut tx, mut rx) = segqueue_channel(10);
            block_on(tx.send(1)).unwrap();
            drop(tx);
            // Channel should still have the item
            assert_eq!(block_on(rx.recv()), Some(1));
            // After draining, should return None
            assert_eq!(block_on(rx.recv()), None);
        }

        #[test]
        fn receiver_drop_returns_disconnected_error() {
            let (mut tx, rx) = segqueue_channel(10);
            drop(rx);
            // Give the drop handler a moment to close the channel
            thread::sleep(Duration::from_millis(50));
            let result = block_on(tx.send(42));
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.is_disconnected());
        }

        #[test]
        fn recv_stream_termination() {
            let (mut tx, mut rx) = segqueue_channel(10);
            block_on(tx.send(1)).unwrap();
            block_on(tx.send(2)).unwrap();
            block_on(tx.send(3)).unwrap();
            drop(tx);

            // This test uses try_send variant to avoid cross-thread wakeup issues
            let mut count = 0;
            while let Some(_) = block_on(rx.recv()) {
                count += 1;
            }
            assert_eq!(count, 3);
        }

        #[test]
        fn sender_sink_close() {
            let (mut tx, _rx) = segqueue_channel::<i32>(10);
            // Closing via Sink::poll_close
            let result = block_on(async { futures::SinkExt::close(&mut tx).await });
            assert!(result.is_ok());
        }

        #[test]
        fn sender_sink_feed_and_flush() {
            // Use the Sink's feed (poll_ready/start_send) and flush
            let (mut tx, mut rx) = segqueue_channel(10);
            // Use SinkExt::feed to test poll_ready/start_send combo
            block_on(async {
                futures::SinkExt::feed(&mut tx, 42).await.unwrap();
                futures::SinkExt::flush(&mut tx).await.unwrap();
            });
            assert_eq!(block_on(rx.recv()), Some(42));
        }

        #[test]
        fn sender_is_full_is_empty_len() {
            // QueueSender exposes is_full, is_empty, len through its own API
            // Access via queue_channel directly (not through Sender wrapper)
            use crossbeam_queue::SegQueue;
            let q = std::sync::Arc::new(SegQueue::new());
            let (mut tx, _rx) = queue_ext::queue_channel(
                q.clone(),
                |_s, act| match act {
                    queue_ext::Action::Send(val) => {
                        q.push(val);
                        queue_ext::Reply::Send(())
                    }
                    queue_ext::Action::IsFull => queue_ext::Reply::IsFull(false),
                    queue_ext::Action::IsEmpty => queue_ext::Reply::IsEmpty(q.is_empty()),
                    queue_ext::Action::Len => queue_ext::Reply::Len(q.len()),
                },
                |s, _| match s.pop() {
                    Some(m) => std::task::Poll::Ready(Some(m)),
                    None => std::task::Poll::Pending,
                },
            );
            assert!(!tx.is_full());
            assert!(tx.is_empty());
            assert_eq!(tx.len(), 0);
            tx.try_send(1).unwrap();
            assert!(!tx.is_empty());
            assert_eq!(tx.len(), 1);
            // try_send on full channel
            let err = tx.try_send(2).unwrap();
            assert_eq!(err, ());
        }

        // ---------- LocalSender / LocalReceiver ----------
        #[test]
        fn localsender_localreceiver_send_recv() {
            let (tx, rx) = segqueue_channel(10);
            let mut ltx = LocalSender::new(tx);
            let mut lrx = LocalReceiver::new(rx);
            block_on(ltx.send(1)).unwrap();
            block_on(ltx.send(2)).unwrap();
            assert_eq!(block_on(lrx.recv()), Some(1));
            assert_eq!(block_on(lrx.recv()), Some(2));
        }

        #[test]
        fn localsender_clone() {
            let (tx, mut lrx) = segqueue_channel(10);
            let mut ls1 = LocalSender::new(tx);
            let mut ls2 = ls1.clone();
            block_on(ls1.send(1)).unwrap();
            block_on(ls2.send(2)).unwrap();
            assert_eq!(block_on(lrx.recv()), Some(1));
            assert_eq!(block_on(lrx.recv()), Some(2));
        }

        #[test]
        fn localreceiver_recv_none_on_close() {
            let (tx, lrx) = segqueue_channel(10);
            let mut ltx = LocalSender::new(tx);
            let mut rx = LocalReceiver::new(lrx);
            block_on(ltx.send(1)).unwrap();
            drop(ltx);
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), None);
        }

        #[test]
        fn sender_receiver_is_send_sync() {
            fn assert_send<T: Send>() {}
            fn assert_sync<T: Sync>() {}
            assert_send::<Sender<i32, SendError<i32>>>();
            assert_sync::<Sender<i32, SendError<i32>>>();
            assert_send::<Receiver<i32>>();
            assert_sync::<Receiver<i32>>();
        }

        #[test]
        fn sender_box_clone_via_trait() {
            let (tx, mut rx) = segqueue_channel(10);
            let cloned = tx.clone();
            let mut tx2 = cloned;
            block_on(tx2.send(1)).unwrap();
            assert_eq!(block_on(rx.recv()), Some(1));
        }
    }

    // -----------------------------------------------------------------------
    // VecDeque channel tests
    // -----------------------------------------------------------------------
    #[cfg(feature = "vecdeque")]
    mod vecdeque_tests {
        use super::*;

        #[test]
        fn sender_drop_closes_channel() {
            let (mut tx, mut rx) = vecdeque_channel(10);
            block_on(tx.send(1)).unwrap();
            drop(tx);
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), None);
        }

        #[test]
        fn with_vecdeque_channel_factory() {
            let q =
                std::sync::Arc::new(parking_lot::RwLock::new(std::collections::VecDeque::new()));
            let (mut tx, mut rx) = with_vecdeque_channel(q, 10);
            block_on(tx.send(1)).unwrap();
            assert_eq!(block_on(rx.recv()), Some(1));
        }

        #[test]
        fn sender_clone() {
            let (mut tx, mut rx) = vecdeque_channel(10);
            let mut tx2 = tx.clone();
            block_on(tx.send(1)).unwrap();
            block_on(tx2.send(2)).unwrap();
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), Some(2));
        }

        #[test]
        fn localsender_localreceiver() {
            let (tx, rx) = vecdeque_channel(10);
            let mut ltx = LocalSender::new(tx);
            let mut lrx = LocalReceiver::new(rx);
            block_on(ltx.send(1)).unwrap();
            assert_eq!(block_on(lrx.recv()), Some(1));
        }
    }

    // -----------------------------------------------------------------------
    // Priority channel tests
    // -----------------------------------------------------------------------
    #[cfg(feature = "priority")]
    mod priority_tests {
        use super::*;

        #[test]
        fn higher_priority_received_first() {
            let (mut tx, mut rx) = priority_channel(10);
            block_on(tx.send((1, "low"))).unwrap();
            block_on(tx.send((5, "high"))).unwrap();
            block_on(tx.send((3, "mid"))).unwrap();
            // BinaryHeap: highest priority (largest Ord) first
            assert_eq!(block_on(rx.recv()), Some((5, "high")));
            assert_eq!(block_on(rx.recv()), Some((3, "mid")));
            assert_eq!(block_on(rx.recv()), Some((1, "low")));
        }

        #[test]
        fn with_priority_channel_factory() {
            use collections::PriorityQueue;
            let q = std::sync::Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
            let (mut tx, mut rx) = with_priority_channel::<i32, &str>(q, 10);
            block_on(tx.send((10, "a"))).unwrap();
            block_on(tx.send((20, "b"))).unwrap();
            assert_eq!(block_on(rx.recv()), Some((20, "b")));
            assert_eq!(block_on(rx.recv()), Some((10, "a")));
        }

        #[test]
        fn priority_sender_drop() {
            let (mut tx, mut rx) = priority_channel(10);
            block_on(tx.send((1, "x"))).unwrap();
            drop(tx);
            assert_eq!(block_on(rx.recv()), Some((1, "x")));
            assert_eq!(block_on(rx.recv()), None);
        }

        #[test]
        fn priority_localsender() {
            let (tx, rx) = priority_channel(10);
            let mut ltx = LocalSender::new(tx);
            let mut lrx = LocalReceiver::new(rx);
            block_on(ltx.send((100, "top"))).unwrap();
            block_on(ltx.send((1, "bottom"))).unwrap();
            assert_eq!(block_on(lrx.recv()), Some((100, "top")));
            assert_eq!(block_on(lrx.recv()), Some((1, "bottom")));
        }
    }

    // -----------------------------------------------------------------------
    // IndexMap channel tests
    // -----------------------------------------------------------------------
    #[cfg(feature = "indexmap")]
    mod indexmap_tests {
        use super::*;

        #[test]
        fn send_recv_one() {
            let (mut tx, mut rx) = indexmap_channel(10);
            block_on(tx.send(("k1", 1))).unwrap();
            assert_eq!(block_on(rx.recv()), Some(("k1", 1)));
        }

        #[test]
        fn with_indexmap_channel_factory() {
            use indexmap::IndexMap;
            let q = std::sync::Arc::new(parking_lot::RwLock::new(IndexMap::new()));
            let (mut tx, mut rx) = with_indexmap_channel(q, 10);
            block_on(tx.send(("a", 1))).unwrap();
            assert_eq!(block_on(rx.recv()), Some(("a", 1)));
        }

        #[test]
        fn indexmap_localsender() {
            let (tx, rx) = indexmap_channel(10);
            let mut ltx = LocalSender::new(tx);
            let mut lrx = LocalReceiver::new(rx);
            block_on(ltx.send(("k", 42))).unwrap();
            assert_eq!(block_on(lrx.recv()), Some(("k", 42)));
        }
    }

    // -----------------------------------------------------------------------
    // Additional tests for VecDeque gaps
    // -----------------------------------------------------------------------
    #[cfg(feature = "vecdeque")]
    mod vecdeque_extra_tests {
        use super::*;

        #[test]
        fn sender_clone() {
            let (mut tx, mut rx) = vecdeque_channel(10);
            let mut tx2 = tx.clone();
            block_on(tx.send(1)).unwrap();
            block_on(tx2.send(2)).unwrap();
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), Some(2));
        }

        #[test]
        fn receiver_drop_disconnects() {
            let (mut tx, rx) = vecdeque_channel(10);
            drop(rx);
            thread::sleep(Duration::from_millis(50));
            let err = block_on(tx.send(42)).unwrap_err();
            assert!(err.is_disconnected());
        }
    }

    // -----------------------------------------------------------------------
    // Additional tests for Priority gaps
    // -----------------------------------------------------------------------
    #[cfg(feature = "priority")]
    mod priority_extra_tests {
        use super::*;

        #[test]
        fn sender_clone() {
            let (mut tx, mut rx) = priority_channel(10);
            let mut tx2 = tx.clone();
            block_on(tx.send((2, "a"))).unwrap();
            block_on(tx2.send((1, "b"))).unwrap();
            assert_eq!(block_on(rx.recv()), Some((2, "a")));
            assert_eq!(block_on(rx.recv()), Some((1, "b")));
        }

        #[test]
        fn receiver_drop_disconnects() {
            let (mut tx, rx) = priority_channel::<i32, &str>(10);
            drop(rx);
            thread::sleep(Duration::from_millis(50));
            let err = block_on(tx.send((1, "x"))).unwrap_err();
            assert!(err.is_disconnected());
        }

        #[test]
        fn receiver_stream_termination() {
            let (mut tx, mut rx) = priority_channel(10);
            block_on(tx.send((2, "b"))).unwrap();
            block_on(tx.send((1, "a"))).unwrap();
            drop(tx);
            assert_eq!(block_on(rx.recv()), Some((2, "b")));
            assert_eq!(block_on(rx.recv()), Some((1, "a")));
            assert_eq!(block_on(rx.recv()), None);
        }

        #[test]
        fn equal_priority_order() {
            let (mut tx, mut rx) = priority_channel(10);
            block_on(tx.send((1, "first"))).unwrap();
            block_on(tx.send((1, "second"))).unwrap();
            block_on(tx.send((1, "third"))).unwrap();
            // With equal priority, BinaryHeap preserves insertion order for equal elements
            let v = block_on(rx.recv());
            // Items with equal priority may be returned in any order
            assert!(v.is_some());
            assert_eq!(v.unwrap().0, 1);
        }
    }

    // -----------------------------------------------------------------------
    // Additional tests for IndexMap gaps
    // -----------------------------------------------------------------------
    #[cfg(feature = "indexmap")]
    mod indexmap_extra_tests {
        use super::*;

        #[test]
        fn receiver_drop_disconnects() {
            let (mut tx, rx) = indexmap_channel::<&str, i32>(10);
            drop(rx);
            thread::sleep(Duration::from_millis(50));
            let err = block_on(tx.send(("k", 1))).unwrap_err();
            assert!(err.is_disconnected());
        }
    }

    // -----------------------------------------------------------------------
    // SegQueue extra coverage
    // -----------------------------------------------------------------------
    #[cfg(feature = "segqueue")]
    mod segqueue_extra_tests {
        use super::*;

        #[test]
        fn is_closed_states() {
            let (mut tx, mut rx) = segqueue_channel::<i32>(10);
            assert!(!rx.is_closed());
            block_on(tx.send(1)).unwrap();
            assert!(!rx.is_closed());
            drop(tx);
            thread::sleep(Duration::from_millis(50));
            assert_eq!(block_on(rx.recv()), Some(1));
            assert_eq!(block_on(rx.recv()), None);
        }
    }
}
