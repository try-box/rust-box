use std::marker::Unpin;

use futures::Stream;

#[allow(unreachable_pub)]
pub use self::limiter::{IntoLimiter, Limiter};

mod limiter;

impl<T: ?Sized> LimiterExt for T where T: Stream {}

pub trait LimiterExt: futures::StreamExt {
    fn limiter<L>(self, l: L) -> IntoLimiter<Self, L>
        where
            Self: Sized + Stream + Unpin,
            L: Limiter + Unpin,
    {
        assert_stream::<Self::Item, _>(IntoLimiter::new(self, l))
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
