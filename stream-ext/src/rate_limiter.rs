use futures::FutureExt;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use leaky_bucket::{AcquireOwned, RateLimiter};

use super::Limiter;

pub struct LeakyBucketRateLimiter {
    limiter: std::sync::Arc<RateLimiter>,
    acquire: Pin<Box<AcquireOwned>>,
}

impl fmt::Debug for LeakyBucketRateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeakyBucketRateLimiter").finish()
    }
}

impl LeakyBucketRateLimiter {
    pub(super) fn new(limiter: RateLimiter) -> Self {
        let limiter = Arc::new(limiter);
        let acquire = Box::pin(limiter.clone().acquire_owned(1));
        Self { limiter, acquire }
    }
}

impl Limiter for LeakyBucketRateLimiter {
    fn acquire(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        let poll = self.acquire.poll_unpin(cx);
        if poll.is_ready() {
            futures::ready!(poll);
            self.acquire = Box::pin(self.limiter.clone().acquire_owned(1));
            Poll::Ready(Some(()))
        } else {
            Poll::Pending
        }
    }
}
