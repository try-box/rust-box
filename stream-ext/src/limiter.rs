use std::fmt;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::ready;
use futures::stream::{FusedStream, Stream};

pub trait Limiter {
    fn acquire(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>>;
}

pub struct IntoLimiter<St, L> {
    stream: St,
    limiter: L,
}

impl<St, L> fmt::Debug for IntoLimiter<St, L>
    where
        St: fmt::Debug,
        L: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IntoLimiter")
            .field("stream", &self.stream)
            .field("limiter", &self.limiter)
            .finish()
    }
}

impl<St, L> IntoLimiter<St, L> {
    pub(crate) fn new(stream: St, limiter: L) -> Self {
        Self { stream, limiter }
    }
}

impl<St, L> FusedStream for IntoLimiter<St, L>
    where
        St: FusedStream + Unpin,
        L: Limiter + Unpin,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, F> Stream for IntoLimiter<St, F>
    where
        St: Stream + Unpin,
        F: Limiter + Unpin,
{
    type Item = St::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(Pin::new(&mut self.limiter).acquire(cx)) {
            Some(()) => Pin::new(&mut self.stream).poll_next(cx),
            None => Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}
