#![allow(unused)]
#![allow(dead_code)]

use futures::{FutureExt, SinkExt, stream, StreamExt};
use futures::{Future, Stream};
use rust_box::queue_ext::{Action, QueueExt, Reply, Waker};
use rust_box::stream_ext::{IntoLimiter, Limiter, LimiterExt};
use std::collections::*;
use std::fmt::Debug;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::task::spawn_local;
use tokio::time::{Instant, Sleep};

fn main() {
    std::env::set_var("RUST_LOG", "stream_ext=info");
    env_logger::init();

    let runner = async move {
        futures::future::join3(
            test_with_limiter(),
            test_with_leaky_bucket_limiter(),
            test_with_governor_limiter(),
        )
            .await;

        // test_with_limiter().await;
        // test_with_leaky_bucket_limiter().await;
        // test_with_governor_limiter().await;
    };
    // async_std::task::block_on(runner);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
    // tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_with_limiter() {
    let s = stream::repeat(1);

    const INTERVAL: Duration = Duration::from_millis(500);
    #[derive(Debug)]
    struct TheLimiter {
        sleep: Pin<Box<Sleep>>,
    }

    impl TheLimiter {
        fn new() -> Self {
            Self {
                sleep: Box::pin(tokio::time::sleep(INTERVAL)),
            }
        }
    }
    impl Limiter for TheLimiter {
        fn acquire(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>> {
            match self.sleep.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    self.sleep.as_mut().reset(Instant::now() + INTERVAL);
                    Poll::Ready(Some(()))
                }
            }
        }
    }

    let mut s = s.limiter(TheLimiter::new());

    log::info!("stream limiter: {:?}", s);

    while let Some(item) = s.next().await {
        log::info!("recv limiter: {:?}", item);
    }
}

async fn test_with_leaky_bucket_limiter() {
    // let s = get_queue_stream(20);
    let s = stream::repeat(2);

    let rate_limiter = leaky_bucket::RateLimiter::builder()
        .initial(3)
        .refill(1)
        .interval(Duration::from_millis(500))
        .max(100)
        .fair(false)
        .build();

    let mut s = s.leaky_bucket_limiter(rate_limiter);

    while let Some(item) = s.next().await {
        log::info!("recv leaky_bucket_limiter: {:?}", item);
    }
}

async fn test_with_governor_limiter() {
    use futures::Stream;
    use governor::state::StreamRateLimitExt;
    use governor::{Quota, RateLimiter, RatelimitedStream};
    use nonzero_ext::nonzero;

    let s = stream::repeat(3);
    // let s = get_queue_stream(10);
    let lim = RateLimiter::direct(Quota::per_second(nonzero!(2u32)));
    // let mut s = inner.ratelimit_stream(&lim);

    let mut s = s.governor_limiter(&lim);

    while let Some(item) = s.next().await {
        log::info!("recv governor_limiter: {:?}", item);
    }
}

fn get_queue_stream(max: i32) -> impl Stream<Item=i32> + Debug {
    use crossbeam_queue::SegQueue;

    let s = Rc::new(SegQueue::default()).queue_stream::<i32, _>(|s, _| {
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().sender::<i32, _, _>(|s, act| match act {
        Action::Send(item) => Reply::Send(s.push(item)),
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
    });

    spawn_local(async move {
        for i in 0..max {
            tx.send(i).await;
        }
    });

    s
}
