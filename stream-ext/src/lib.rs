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
    ) -> governor::RatelimitedStream<'_, Self, D, C, MW>
    where
        D: governor::state::DirectStateStore,
        C: governor::clock::Clock + governor::clock::ReasonablyRealtime,
        MW: governor::middleware::RateLimitingMiddleware<
            C::Instant,
            NegativeOutcome = governor::NotUntil<<C as governor::clock::Clock>::Instant>,
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
    S: Stream<Item = T>,
{
    stream
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::*;
    use futures::StreamExt;

    struct NoLimiter;

    impl Limiter for NoLimiter {
        fn acquire(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<()>> {
            Poll::Ready(Some(()))
        }
    }

    #[test]
    fn test_into_limiter_basic() {
        use futures::executor::block_on;

        let stream = futures::stream::iter(1..=5i32);
        let into_limiter = IntoLimiter::new(stream, NoLimiter);
        let results: Vec<i32> = block_on(into_limiter.collect());
        assert_eq!(results, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_into_limiter_acquire_blocks() {
        struct AlternatingLimiter(std::cell::Cell<bool>);

        impl Limiter for AlternatingLimiter {
            fn acquire(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<()>> {
                let blocked = self.0.get();
                self.0.set(!blocked);
                if blocked {
                    Poll::Pending
                } else {
                    Poll::Ready(Some(()))
                }
            }
        }

        let stream = futures::stream::iter(1..=3i32);
        let into_limiter = IntoLimiter::new(stream, AlternatingLimiter(std::cell::Cell::new(true)));

        futures::pin_mut!(into_limiter);

        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        // First poll: limiter returns Pending (blocks)
        assert_eq!(into_limiter.as_mut().poll_next(&mut cx), Poll::Pending);

        // Second poll: limiter returns Ready -> stream yields item
        assert_eq!(
            into_limiter.as_mut().poll_next(&mut cx),
            Poll::Ready(Some(1))
        );
    }

    #[test]
    fn test_limiter_trait_object() {
        let mut limiter = NoLimiter;
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        // Create Pin<&mut NoLimiter> then coerce to Pin<&mut dyn Limiter>
        let pinned = Pin::new(&mut limiter);
        let pinned_dyn: Pin<&mut dyn Limiter> = pinned;
        assert_eq!(pinned_dyn.acquire(&mut cx), Poll::Ready(Some(())));
    }

    #[test]
    fn test_limiter_ext() {
        use futures::executor::block_on;

        let stream = futures::stream::iter(1..=3i32).limiter(NoLimiter);
        let results: Vec<i32> = block_on(stream.collect());
        assert_eq!(results, vec![1, 2, 3]);
    }

    #[cfg(feature = "leaky-bucket")]
    #[test]
    fn test_leaky_bucket_limiter() {
        use futures::executor::block_on;
        use std::time::Duration;

        let rate_limiter = leaky_bucket::RateLimiter::builder()
            .initial(10)
            .max(10)
            .interval(Duration::from_millis(100))
            .build();

        let stream = futures::stream::iter(1..=5i32);
        let limited = stream.leaky_bucket_limiter(rate_limiter);
        let results: Vec<i32> = block_on(limited.collect());
        assert_eq!(results, vec![1, 2, 3, 4, 5]);
    }

    #[cfg(feature = "governor")]
    #[test]
    fn test_governor_limiter() {
        use futures::executor::block_on;
        use futures::StreamExt;

        let rate_limiter = governor::RateLimiter::direct(governor::Quota::per_second(
            std::num::NonZeroU32::new(100).unwrap(),
        ));
        let stream = futures::stream::iter(1..=5i32);
        let limited = stream.governor_limiter(&rate_limiter);
        let results: Vec<i32> = block_on(limited.collect());
        assert_eq!(results, vec![1, 2, 3, 4, 5]);
    }
}
