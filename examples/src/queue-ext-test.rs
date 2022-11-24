#![allow(unused)]
#![allow(dead_code)]

use std::collections::*;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::{AsyncWriteExt, FutureExt, SinkExt, stream, StreamExt};
use rust_box::queue_ext::{Action, QueueExt, Reply, Waker};
use tokio::task::spawn_local;
use tokio::time::sleep;

fn main() {
    std::env::set_var("RUST_LOG", "queue_ext=info");
    env_logger::init();

    let runner = async move {
        test_with_vec_deque().await;
        test_with_indexmap().await;
        test_with_heep().await;
        test_with_crossbeam_segqueue().await;
        test_with_crossbeam_arrqueue().await;
        test_queue_channel_with_segqueue().await;
        test_with_queue_stream().await;
    };
    // async_std::task::block_on(runner);
    tokio::task::LocalSet::new().block_on(&tokio::runtime::Runtime::new().unwrap(), runner);
    // tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_with_vec_deque() {
    use parking_lot::RwLock;
    let mut s = Rc::new(RwLock::new(VecDeque::new())).queue_stream(|s, _| {
        let mut s = s.write();
        match s.pop_front() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(val) => {
            s.write().push_back(val);
            Reply::Send(())
        }
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
        Action::Len => Reply::Len(s.read().len()),
    });

    spawn_local(async move {
        for i in 0..10 {
            let res = tx.send(i).await.unwrap();
            sleep(Duration::from_millis(1)).await;
        }
        tx.close().await;
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


async fn test_with_indexmap() {
    use parking_lot::RwLock;
    use indexmap::IndexMap;

    let mut s = Rc::new(RwLock::new(IndexMap::new())).queue_stream::<(i32, i32), _>(|s, _| {
        let mut s = s.write();
        match s.pop() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        }
    });

    let mut tx = s
        .clone()
        .queue_sender::<(i32, i32), _, _>(|s, act| match act {
            Action::Send((key, val)) => {
                let mut s = s.write();
                //Remove this entry if it already exists
                if s.contains_key(&key) {
                    s.remove(&key);
                }
                s.insert(key, val);
                Reply::Send(())
            }
            Action::IsFull => Reply::IsFull(false),
            Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
            Action::Len => Reply::Len(s.read().len()),
        });

    let mut tx1 = tx.clone();
    spawn_local(async move {
        for i in 0..30 {
            tokio::time::sleep(Duration::from_millis(1)).await;
            tx1.send((i % 10, i*2)).await.unwrap();
        }
        tx1.close().await;
    });

    while let Some(item) = s.next().await {
        log::info!("test indexmap: {:?}, len: {}", item, tx.len());
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn test_with_heep() {
    use parking_lot::RwLock;

    let mut s = Rc::new(RwLock::new(BinaryHeap::new())).queue_stream(|s, _| {
        let mut s = s.write();
        match s.pop() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.write().push(item)),
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.read().is_empty()),
        Action::Len => Reply::Len(s.read().len()),
    });

    let mut tx1 = tx.clone();
    spawn_local(async move {
        for i in 0..100 {
            tx1.send(i).await.unwrap();
        }
        tx1.close().await;
    });

    while let Some(item) = s.next().await {
        log::info!("test BinaryHeap: {:?}, len: {} {}", item, s.read().len(), tx.len());
    }
}

async fn test_with_crossbeam_segqueue() {
    use crossbeam_queue::SegQueue;

    let mut s = Rc::new(SegQueue::default()).queue_stream(|s, _| {
        match s.pop() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.push(item)),
        Action::IsFull => Reply::IsFull(false),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
        Action::Len => Reply::Len(s.len()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tx.send(i).await.unwrap();
        }
        tx.close().await;
    });

    while let Some(item) = s.next().await {
        log::info!("test SegQueue: {:?}, len: {}", item, s.len());
    }
}

async fn test_with_crossbeam_arrqueue() {
    use crossbeam_queue::ArrayQueue;

    let mut s = Rc::new(ArrayQueue::new(10)).queue_stream(|s, _| {
        match s.pop() {
            Some(m) => Poll::Ready(Some(m)),
            None => Poll::Pending,
        }
    });

    let mut tx = s.clone().queue_sender(|s, act| match act {
        Action::Send(item) => Reply::Send(s.push(item)),
        Action::IsFull => Reply::IsFull(s.is_full()),
        Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
        Action::Len => Reply::Len(s.len()),
    });

    spawn_local(async move {
        for i in 0..100 {
            tx.send(i).await.unwrap();
        }
        tx.close().await;
    });

    while let Some(item) = s.next().await {
        log::info!("recv ArrayQueue: {:?}, len: {}", item, s.len());
    }
}

async fn test_queue_channel_with_segqueue() {
    use crossbeam_queue::SegQueue;

    let (mut tx, mut rx) = Rc::new(SegQueue::default()).queue_channel(
        |s, act| match act {
            Action::Send(item) => Reply::Send(s.push(item)),
            Action::IsFull => Reply::IsFull(false),
            Action::IsEmpty => Reply::IsEmpty(s.is_empty()),
            Action::Len => Reply::Len(s.len()),
        },
        |s, _| {
            match s.pop() {
                Some(m) => Poll::Ready(Some(m)),
                None => Poll::Pending,
            }
        },
    );

    spawn_local(async move {
        for i in 0..100 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            tx.send(i).await.unwrap();
        }
        tx.close().await;
    });

    while let Some(item) = rx.next().await {
        log::info!("test queue_channel: {:?}, len: {}", item, rx.len());
    }
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
        q.close();
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

