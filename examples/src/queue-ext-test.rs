#![allow(unused)]
#![allow(dead_code)]

use futures::{AsyncWriteExt, FutureExt, SinkExt, stream, StreamExt};
use rust_box::queue_ext::{QueueExt, Waker};
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
        let test_futs1 = futures::future::join5(
            test_with_queue_stream(),
            test_with_vec_deque(),
            test_with_linked_hash_map(),
            test_with_linked_hash_map_async_lock(),
            test_with_heep(),
        );

        let test_futs2 = futures::future::join(
            test_with_crossbeam_segqueue(),
            test_with_crossbeam_arrqueue(),
        ).await;

        // futures::future::join(test_futs1, test_futs2).await;

        // test_with_queue_stream().await;
        // test_with_vec_deque().await;
        // test_with_linked_hash_map().await;
        // test_with_linked_hash_map_async_lock().await;
        // test_with_heep().await;
        // test_with_crossbeam_segqueue().await;
        // test_with_crossbeam_arrqueue().await;
    };
    // async_std::task::block_on(runner);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
    // tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_futures_channel() {
    use futures::channel::mpsc::unbounded;
    use futures::channel::mpsc::Receiver;

    let (mut tx, mut rx) = unbounded::<i32>();

    spawn_local(async move {
        for i in 0..10 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let res = tx.send(i).await;
            if let Err(e) = res {
                log::info!("send error, {:?}", e);
            }
            if i == 5 {
                tx.close_channel();
                // tx.close().await;
                // tx.disconnect();
            }
        }
    });

    let mut count = 0;
    while let Some(item) = rx.next().await {
        count += 1;
        log::info!("test futures_channel: {:?}, count: {}", item, count);
    }
    log::info!("end ...");
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
            q.wake();
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

    let mut tx = s.clone().sender(|s, v| {
        s.write().push_back(v);
        (1, 2)
    });

    spawn_local(async move {
        for i in 0..10 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let res = tx.send(i);
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

    let mut tx = s.clone().sender::<(i32, i32), _, _>(|s, v| {
        let mut s = s.write();
        let key = v.0;
        if s.contains_key(&key) {
            s.remove(&key);
        }
        s.insert(key, v.1);
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send((i % 10, i));
        }
    });

    while let Some(item) = s.next().await {
        log::info!("test linked_hash_map: {:?}, len: {}", item, s.read().len());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn test_with_linked_hash_map_async_lock() {
    use linked_hash_map::LinkedHashMap;
    use tokio::sync::RwLock;
    let m: LinkedHashMap<i32, i32> = LinkedHashMap::new();
    let mut s = Rc::new(RwLock::new(m)).queue_stream(|s, cx| {
        let s = s.clone();
        let res = async move {
            let mut s = s.write().await;
            if s.is_empty() {
                None
            } else {
                match s.pop_front() {
                    Some(m) => Some(m),
                    None => None,
                }
            }
        };
        use futures_lite::StreamExt as _;
        let mut res = Box::pin(res.into_stream());
        match res.poll_next(cx) {
            Poll::Ready(Some(Some(m))) => Poll::Ready(Some(m)),
            _ => Poll::Pending,
        }
    });

    let mut tx = s.clone().sender(|s, v: (i32, i32)| {
        let s = s.clone();
        async move {
            let mut s = s.write().await;
            let key = v.0;
            if s.contains_key(&key) {
                s.remove(&key);
            }
            s.insert(key, v.1);
        }
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let res = tx.send((i % 10, i)).await;
        }
    });

    while let Some(item) = s.next().await {
        log::info!("test linked_hash_map async_lock: {:?}", item);
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

    let mut tx = s.clone().sender(|s, v| {
        s.write().push(v);
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i);
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

    let mut tx = s.clone().sender(|s, v| {
        s.push(v);
    });

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i);
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

    let mut tx = s.clone().sender(|s, v| s.push(v));

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let res = tx.send(i);
            if let Err(e) = res {
                log::warn!("test ArrayQueue send error, {:?}", e);
            }
        }
    });

    while let Some(item) = s.next().await {
        log::info!("recv ArrayQueue: {:?}, len: {}", item, s.len());
    }
}
