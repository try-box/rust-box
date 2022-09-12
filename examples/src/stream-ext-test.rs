#![allow(unused)]
#![allow(dead_code)]

use futures::{FutureExt, SinkExt, stream, StreamExt};
use leaky_bucket::{AcquireOwned, RateLimiter};
use std::collections::*;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::task::spawn_local;
use try_box::stream_ext::{IntoLimiter, Limiter, LimiterExt};
use try_box::queue_ext::{QueueExt, Waker};

fn main() {
    std::env::set_var("RUST_LOG", "stream_ext=info");
    env_logger::init();

    let runner = async move {
        // test_with_limiter().await;
        test_with_limiter2().await;
    };
    // async_std::task::block_on(runner);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
    // tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_with_limiter() {
    let s = stream::iter(0..10);

    let mut s = s.limiter(StreamRateLimiter::new());

    let mut count = 0;
    while let Some(item) = s.next().await {
        count += 1;
        log::info!("recv limiter: {:?}, count: {}", item, count);
    }
}

async fn test_with_limiter2() {
    use crossbeam_queue::SegQueue;

    let s = Rc::new(SegQueue::default()).into_stream::<i32, _>(|s, _| {
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().sender::<i32, _, _>(|s, v| {
        s.push(v);
    });

    spawn_local(async move {
        for i in 0..10 {
            tx.send(i);
        }
    });

    let mut s = s.limiter(StreamRateLimiter::new());

    let mut count = 0;
    while let Some(item) = s.next().await {
        count += 1;
        log::info!("recv limiter: {:?}, count: {}", item, count);
    }
}

struct StreamRateLimiter {
    inner: Arc<RateLimiter>,
    acquire: Pin<Box<AcquireOwned>>,
}

impl Default for StreamRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamRateLimiter {
    fn new() -> Self {
        let inner = Arc::new(
            RateLimiter::builder()
                .initial(3)
                .refill(1)
                .max(3)
                .interval(std::time::Duration::from_millis(1000))
                // .fair(false)
                .build(),
        );
        let acquire = Box::pin(inner.clone().acquire_owned(1));
        Self { inner, acquire }
    }
}

impl Limiter for StreamRateLimiter {
    fn acquire(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
        let poll = self.acquire.poll_unpin(cx);
        if poll.is_ready() {
            futures::ready!(poll);
            self.acquire = Box::pin(self.inner.clone().acquire_owned(1));
            Poll::Ready(Some(()))
        } else {
            Poll::Pending
        }
    }
}
