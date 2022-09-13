use std::marker::Unpin;

use futures::Stream;

#[allow(unreachable_pub)]
pub use self::limiter::{IntoLimiter, Limiter};

#[cfg(feature = "leaky-bucket")]
mod rate_limiter;

mod limiter;

impl<T: ?Sized> LimiterExt for T where T: Stream {}

pub trait LimiterExt: Stream {

    #[inline]
    fn limiter<L>(self, l: L) -> IntoLimiter<Self, L>
        where
            Self: Sized + Stream + Unpin,
            L: Limiter + Unpin,
    {
        assert_stream::<Self::Item, _>(IntoLimiter::new(self, l))
    }

    #[cfg(feature = "leaky-bucket")]
    #[inline]
    fn leaky_bucket_limiter(
        self,
        rate_limiter: leaky_bucket::RateLimiter,
    ) -> IntoLimiter<Self, rate_limiter::LeakyBucketRateLimiter>
        where
            Self: Sized + Stream + Unpin,
    {
        let l = rate_limiter::LeakyBucketRateLimiter::new(rate_limiter);
        assert_stream::<Self::Item, _>(IntoLimiter::new(self, l))
    }

    #[cfg(feature = "governor")]
    #[inline]
    fn governor_limiter<D, C, MW>(
        self,
        rate_limiter: &governor::RateLimiter<governor::state::NotKeyed, D, C, MW>,
    ) -> governor::RatelimitedStream<Self, D, C, MW>
        where
            D: governor::state::DirectStateStore,
            C: governor::clock::Clock + governor::clock::ReasonablyRealtime,
            MW: governor::middleware::RateLimitingMiddleware<
                C::Instant,
                NegativeOutcome=governor::NotUntil<<C as governor::clock::Clock>::Instant>,
            >,
            Self: Sized + Stream + Unpin,
            Self::Item: Unpin,
    {
        use governor::state::StreamRateLimitExt;
        assert_stream::<Self::Item, _>(self.ratelimit_stream(rate_limiter))
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
