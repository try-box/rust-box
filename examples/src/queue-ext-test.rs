#![allow(unused)]
#![allow(dead_code)]

use futures::{AsyncWriteExt, FutureExt, SinkExt, stream, StreamExt};
use rust_box::queue_ext::{Action, QueueExt, Reply, Waker};
use std::collections::*;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::task::spawn_local;

fn main() {
    std::env::set_var("RUST_LOG", "queue_ext=info");
    env_logger::init();

    let runner = async move {
        let test_futs1 = futures::future::join4(
            test_with_queue_stream(),
            test_with_vec_deque(),
            test_with_linked_hash_map(),
            test_with_heep(),
        );

        let test_futs2 = futures::future::join(
            test_with_crossbeam_segqueue(),
            test_with_crossbeam_arrqueue(),
        );

        futures::future::join(test_futs1, test_futs2).await;

        // test_with_queue_stream().await;
        // test_with_vec_deque().await;
        // test_with_linked_hash_map().await;
        // test_with_heep().await;
        // test_with_crossbeam_segqueue().await;
        // test_with_crossbeam_arrqueue().await;
    };
    // async_std::task::block_on(runner);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
    // tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_with_queue_stream() {
    use parking_lot::RwLock;
    let mut s = Rc::new(RwLock::new(VecDeque::new())).queue_stream(|s, _| {
        let mut s = s.write();
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop_front() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let q = s.clone();

    spawn_local(async move {
        for i in 0..10 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            q.write().push_back(i);
            q.rx_wake();
        }
    });

    let mut count = 0;
    while let Some(item) = s.next().await {
        count += 1;
        log::info!(
            "test queue_stream: {:?}, len: {}, count: {}",
            item,
            s.read().len(),
            count
        );
    }
}

async fn test_with_vec_deque() {
    use parking_lot::RwLock;
    let mut s = Rc::new(RwLock::new(VecDeque::new())).queue_stream(|s, _| {
        let mut s = s.write();
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop_front() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(val) => {
            s.write().push_back(val);
            Reply::Send(())
        }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
    });

    spawn_local(async move {
        for i in 0..10 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let res = tx.send(i).await.unwrap();
        }
    });

    let mut count = 0;
    while let Some(item) = s.next().await {
        count += 1;
        log::info!(
            "test VecDeque: {:?}, len: {}, count: {}",
            item,
            s.read().len(),
            count
        );
    }
}

async fn test_with_linked_hash_map() {
    use linked_hash_map::LinkedHashMap;
    use parking_lot::RwLock;
    let mut s = Rc::new(RwLock::new(LinkedHashMap::new())).queue_stream::<(i32, i32), _>(|s, _| {
        let mut s = s.write();
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop_front() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().queue_sender::<(i32, i32), _, _>(|s, act| match act {
        Action::Send((key, val)) => {
            let mut s = s.write();
            if s.contains_key(&key) {
                s.remove(&key);
            }
            s.insert(key, val);
            Reply::Send(())
        }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send((i % 10, i)).await.unwrap();
        }
    });

    while let Some(item) = s.next().await {
        log::info!("test linked_hash_map: {:?}, len: {}", item, s.read().len());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn test_with_heep() {
    use parking_lot::RwLock;

    let mut s = Rc::new(RwLock::new(BinaryHeap::new())).queue_stream(|s, _| {
        let mut s = s.write();
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.write().push(item)),
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i).await.unwrap();
        }
    });

    while let Some(item) = s.next().await {
        log::info!("test BinaryHeap: {:?}, len: {}", item, s.read().len());
    }
}

async fn test_with_crossbeam_segqueue() {
    use crossbeam_queue::SegQueue;

    let mut s = Rc::new(SegQueue::default()).queue_stream(|s, _| {
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.push(item)),
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i).await.unwrap();
        }
    });

    while let Some(item) = s.next().await {
        log::info!("test SegQueue: {:?}, len: {}", item, s.len());
    }
}

async fn test_with_crossbeam_arrqueue() {
    use crossbeam_queue::ArrayQueue;

    let mut s = Rc::new(ArrayQueue::new(10)).queue_stream(|s, _| {
        if s.is_empty() {
            Poll::Pending
        } else {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.push(item)),
        Action::IsFull => Reply::IsFull(s.is_full()),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i).await.unwrap();
        }
    });

    while let Some(item) = s.next().await {
        log::info!("recv ArrayQueue: {:?}, len: {}", item, s.len());
    }
}
