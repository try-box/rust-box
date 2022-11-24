use std::task::Poll;
use futures::{Sink, Stream};
use queue_ext::{Action, QueueExt, Reply};

pub use queue_ext::SendError;

///SegQueue based channel
#[cfg(feature = "segqueue")]
pub fn with_segqueue_channel<T>(queue: std::sync::Arc<crossbeam_queue::SegQueue<T>>, bound: usize)
           -> (impl Sink<T, Error = SendError<T>> + Clone, impl Stream<Item=T>) {
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
            |s, _| {
                match s.pop() {
                    Some(m) => Poll::Ready(Some(m)),
                    None => Poll::Pending,
                }
            },
        );
    (tx, rx)
}

///SegQueue based channel
#[cfg(feature = "segqueue")]
pub fn segqueue_channel<T>(bound: usize) -> (impl Sink<T, Error = SendError<T>> + Clone, impl Stream<Item=T>) {
    use crossbeam_queue::SegQueue;
    use std_ext::ArcExt;
    with_segqueue_channel(SegQueue::default().arc(), bound)
}

///VecDeque based channel
#[cfg(feature = "vecdeque")]
pub fn with_vecdeque_channel<T>(queue: std::sync::Arc<std_ext::RwLock<std::collections::VecDeque<T>>>, bound: usize)
                                -> (impl Sink<T, Error = SendError<T>> + Clone, impl Stream<Item=T>) {
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
    (tx, rx)
                                }

///VecDeque based channel
#[cfg(feature = "vecdeque")]
pub fn vecdeque_channel<T>(bound: usize) -> (impl Sink<T, Error = SendError<T>> + Clone, impl Stream<Item=T>) {
    use std::collections::VecDeque;
    use std_ext::{ArcExt, RwLockExt};
    let queue = VecDeque::default()
        .rwlock()
        .arc();
    with_vecdeque_channel(queue, bound)
}

///Indexmap based channel, remove entry if it already exists
#[cfg(feature = "indexmap")]
pub fn with_indexmap_channel<K, T>(indexmap: std::sync::Arc<std_ext::RwLock<indexmap::IndexMap<K, T>>>, bound: usize)
    -> (impl Sink<(K, T), Error = SendError<(K, T)>> + Clone, impl Stream<Item=(K, T)>)
    where
        K: Eq + std::hash::Hash,
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
    (tx, rx)
}


///Indexmap based channel, remove entry if it already exists
#[cfg(feature = "indexmap")]
pub fn indexmap_channel<K, T>(bound: usize) -> (impl Sink<(K, T), Error = SendError<(K, T)>> + Clone, impl Stream<Item=(K, T)>)
where
    K: Eq + std::hash::Hash,
{
    use indexmap::IndexMap;
    use std_ext::{ArcExt, RwLockExt};
    let map = IndexMap::new()
        .rwlock()
        .arc();
    with_indexmap_channel(map, bound)
}
