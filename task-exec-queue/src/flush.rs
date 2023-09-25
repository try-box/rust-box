use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};

use futures::Sink;

use super::{LocalTaskExecQueue, LocalTaskType, TaskExecQueue, TaskType};

pub struct Flush<'a, Tx, G, D> {
    sink: &'a TaskExecQueue<Tx, G, D>,
}

impl<'a, Tx, G, D> Unpin for Flush<'a, Tx, G, D> {}

impl<'a, Tx, G, D> Flush<'a, Tx, G, D> {
    pub(crate) fn new(sink: &'a TaskExecQueue<Tx, G, D>) -> Self {
        Self { sink }
    }
}

impl<'a, Tx, G, D> Future for Flush<'a, Tx, G, D>
where
    Tx: Clone + Sink<(D, TaskType)> + Unpin + Send + Sync + 'static,
    G: Hash + Eq + Clone + Debug + Send + Sync + 'static,
{
    type Output = Result<(), Tx::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        futures::ready!(Pin::new(&mut this.sink.tx.clone()).poll_flush(cx))?;
        if this.sink.is_active() {
            this.sink.flush_waker.register(cx.waker());
            Poll::Pending
        } else {
            this.sink.is_flushing.store(false, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
}

pub struct LocalFlush<'a, Tx, G, D> {
    sink: &'a LocalTaskExecQueue<Tx, G, D>,
}

impl<'a, Tx, G, D> Unpin for LocalFlush<'a, Tx, G, D> {}

impl<'a, Tx, G, D> LocalFlush<'a, Tx, G, D> {
    pub(crate) fn new(sink: &'a LocalTaskExecQueue<Tx, G, D>) -> Self {
        Self { sink }
    }
}

impl<'a, Tx, G, D> Future for LocalFlush<'a, Tx, G, D>
where
    Tx: Clone + Sink<(D, LocalTaskType)> + Unpin + 'static,
    G: Hash + Eq + Clone + Debug + 'static,
{
    type Output = Result<(), Tx::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        futures::ready!(Pin::new(&mut this.sink.tx.clone()).poll_flush(cx))?;
        if this.sink.is_active() {
            this.sink.flush_waker.register(cx.waker());
            Poll::Pending
        } else {
            this.sink.is_flushing.store(false, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
}
