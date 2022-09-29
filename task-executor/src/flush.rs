use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use futures::Sink;
use futures::task::AtomicWaker;

use super::{Counter, TaskType};

pub struct Flush<Tx, D> {
    sink: Tx,
    waiting_count: Counter,
    active_count: Counter,
    is_flushing: Arc<AtomicBool>,
    w: Arc<AtomicWaker>,
    _d: std::marker::PhantomData<D>,
}

impl<Tx, D> Unpin for Flush<Tx, D> {}

impl<Tx, D> Flush<Tx, D> {
    pub(crate) fn new(
        sink: Tx,
        waiting_count: Counter,
        active_count: Counter,
        is_flushing: Arc<AtomicBool>,
        w: Arc<AtomicWaker>,
    ) -> Self {
        Self {
            sink,
            waiting_count,
            active_count,
            is_flushing,
            w,
            _d: std::marker::PhantomData,
        }
    }
}

impl<Tx, D> Future for Flush<Tx, D>
    where
        Tx: Sink<(D, TaskType)> + Unpin,
{
    type Output = Result<(), Tx::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        futures::ready!(Pin::new(&mut self.sink).poll_flush(cx))?;
        if self.waiting_count.value() > 0 || self.active_count.value() > 0 {
            self.w.register(cx.waker());
            Poll::Pending
        } else {
            self.is_flushing.store(false, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
}
