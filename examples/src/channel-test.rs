#![allow(unused)]
#![allow(dead_code)]
use futures::{stream, AsyncWriteExt, FutureExt, Sink, SinkExt, StreamExt};
//use rust_box::mpsc::channel::{ChildReceiver, ChildSender};
use rust_box::mpsc::{
    indexmap_channel, segqueue_channel, vecdeque_channel, with_segqueue_channel, Receiver,
    SendError, Sender,
};
use std::time::Duration;
use tokio::task::spawn;
use tokio::time::sleep;

// use segqueue_channel as mpsc;
use vecdeque_channel as channel;

fn main() {
    std::env::set_var("RUST_LOG", "channel_test=info");
    env_logger::init();

    let runner = async move {
        test_channel().await;
        test_indexmap_channel().await;
        test_with_segqueue_channel().await;
        test_channel_tx_rx().await;
    };
    // async_std::task::block_on(runner);
    tokio::runtime::Runtime::new().unwrap().block_on(runner);
}

async fn test_channel() {
    /*
    #[derive(Clone)]
    struct S<Tx> {
        tx: Tx,
    }

    impl<Tx: Sink<i32, Error = SendError<i32>> + Clone> S<Tx> {
        fn new(tx: Tx) -> Self {
            Self {
                tx
            }
        }
    }
    */

    let (mut tx, mut rx) = channel::<i32>(100);

    let mut tx1 = tx.clone();
    let mut tx2 = tx.clone();

    spawn(async move {
        for i in 0..100 {
            if let Err(e) = tx1.send(i).await {
                log::warn!("tx1, {:?}", e);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    });

    spawn(async move {
        for i in 100..200 {
            if let Err(e) = tx2.send(i).await {
                log::warn!("tx2, {:?}", e);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    });
    tx.close().await;

    let mut count = 0;
    while let Some(item) = rx.next().await {
        count += 1;
        log::info!(
            "test channel: {:?}, len: {}, count: {}",
            item,
            0, //s.read().len(),
            count
        );
    }
}

async fn test_indexmap_channel() {
    let (mut tx, mut rx) = indexmap_channel::<i32, i32>(100);

    let mut tx1 = tx.clone();
    let mut tx2 = tx.clone();

    spawn(async move {
        for i in 0..100 {
            tx1.send((i % 10, i * 2)).await.unwrap();
            sleep(Duration::from_millis(1)).await;
        }
        tx1.close().await;
    });

    spawn(async move {
        for i in 100..200 {
            tx2.send((i % 10, i * 2)).await.unwrap();
            sleep(Duration::from_millis(2)).await;
        }
        tx2.close().await;
    });

    drop(tx);

    let mut count = 0;
    while let Some(item) = rx.next().await {
        count += 1;
        tokio::time::sleep(Duration::from_millis(20)).await;
        log::info!(
            "test indexmap_channel: {:?}, len: {}, count: {}",
            item,
            0,
            count
        );
    }
}

async fn test_with_segqueue_channel() {
    use crossbeam_queue::SegQueue;
    use rust_box::std_ext::ArcExt;
    let s = SegQueue::default().arc();
    let (mut tx, mut rx) = with_segqueue_channel(s.clone(), 100);

    let mut tx1 = tx.clone();
    let mut tx2 = tx.clone();

    spawn(async move {
        for i in 0..100 {
            if let Err(e) = tx1.send(i).await {
                log::warn!("tx1, {:?}", e);
                break;
            }
        }
    });

    spawn(async move {
        for i in 100..200 {
            if let Err(e) = tx2.send(i).await {
                log::warn!("tx2, {:?}", e);
                break;
            }
        }
    });
    tx.close().await;

    let mut count = 0;
    while let Some(item) = rx.next().await {
        count += 1;
        log::info!(
            "test with segqueue channel: {:?}, len: {}, count: {}",
            item,
            s.len(),
            count
        );
        sleep(Duration::from_millis(1)).await;
    }
}

async fn test_channel_tx_rx() {
    let (mut tx, mut rx) = channel::<i32>(100);

    let mut tx1 = tx.clone();
    let mut tx2 = tx.clone();

    spawn(async move {
        for i in 0..100 {
            if let Err(e) = tx1.send(i).await {
                log::warn!("tx1, {:?}", e);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    });

    spawn(async move {
        for i in 100..200 {
            if let Err(e) = tx2.send(i).await {
                log::warn!("tx2, {:?}", e);
                break;
            }
            sleep(Duration::from_millis(1)).await;
        }
    });
    tx.close().await;

    let mut count = 0;
    while let Some(item) = rx.recv().await {
        count += 1;
        log::info!(
            "test  channel tx rx: {:?}, len: {}, count: {}",
            item,
            0,
            count
        );
    }
}
