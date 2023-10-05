use futures::{Sink, SinkExt, Stream, StreamExt};

use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};

pub use queue_ext::SendError;
#[allow(unused_imports)]
use queue_ext::{Action, QueueExt, Reply, Waker};

///BinaryHeap based channel
#[cfg(feature = "priority")]
#[allow(clippy::type_complexity)]
pub fn with_priority_channel<P: Ord + 'static, T: 'static>(
    queue: std::sync::Arc<std_ext::RwLock<collections::PriorityQueue<P, T>>>,
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
    use collections::PriorityQueue;
    use std_ext::{ArcExt, RwLockExt};
    let queue = PriorityQueue::default().rwlock().arc();
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
    queue: std::sync::Arc<std_ext::RwLock<std::collections::VecDeque<T>>>,
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
    use std_ext::{ArcExt, RwLockExt};
    let queue = VecDeque::default().rwlock().arc();
    with_vecdeque_channel(queue, bound)
}

///Indexmap based channel, remove entry if it already exists
#[cfg(feature = "indexmap")]
#[allow(clippy::type_complexity)]
pub fn with_indexmap_channel<K, T>(
    indexmap: std::sync::Arc<std_ext::RwLock<indexmap::IndexMap<K, T>>>,
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
    use std_ext::{ArcExt, RwLockExt};
    let map = IndexMap::new().rwlock().arc();
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

pub trait ReceiverStream<M>: futures::Stream<Item = M> + Send + Unpin + Waker {}

impl<T, M> ReceiverStream<M> for T where
    T: futures::Stream<Item = M> + Send + Unpin + Waker + 'static
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
        T: futures::Stream<Item = M> + Send + Unpin + Waker + 'static,
    {
        Receiver { rx: Box::new(tx) }
    }

    #[inline]
    pub async fn recv(&mut self) -> Option<M> {
        self.rx.next().await
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
