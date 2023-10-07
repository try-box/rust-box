use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use handy_grpc::client::{Client, Mailbox, Message};
use handy_grpc::Priority;

// cargo run -r --example sender

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "sender=debug,handy_grpc=debug");
    env_logger::init();

    let addr = "[::1]:10000";

    let runner = async move {
        let mut c = Client::new(addr.into()).concurrency_limit(32).build().await;
        let send_count = Arc::new(AtomicUsize::new(0));
        let complete_count = Arc::new(AtomicUsize::new(0));
        let fail_count = Arc::new(AtomicUsize::new(0));
        let mailbox = c.transfer_start(10_000_000).await;
        let msg = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let send_data_futs = async move {
            let send = |mailbox: Mailbox,
                        data: Vec<u8>,
                        p: Priority,
                        sleep_dur: Duration,
                        complete_count: Arc<AtomicUsize>,
                        send_count: Arc<AtomicUsize>,
                        fail_count: Arc<AtomicUsize>| async move {
                let send_fut = |mut mailbox: Mailbox,
                                msg: Message,
                                p: Priority,
                                complete_count: Arc<AtomicUsize>,
                                fail_count: Arc<AtomicUsize>| async move {
                    let msg_bak = msg.clone();
                    let p_bak = p.clone();
                    let mut msg1 = Some(msg);
                    let mut p1 = Some(p);
                    loop {
                        let send_result = mailbox
                            .send_priority(msg1.take().unwrap(), p1.take().unwrap())
                            .await;
                        if let Err(e) = send_result {
                            fail_count.fetch_add(1, Ordering::SeqCst);
                            log::trace!("send error, {:?}", e);
                            if let Some((pp, mm)) = e.into_inner() {
                                msg1 = Some(mm);
                                p1 = Some(pp);
                            } else {
                                log::warn!("send error, into_inner is None");
                                msg1 = Some(msg_bak.clone());
                                p1 = Some(p_bak.clone());
                                //break
                            }
                            tokio::time::sleep(Duration::from_millis(1)).await;
                        } else {
                            complete_count.fetch_add(1, Ordering::SeqCst);
                            break;
                        }
                    }
                };

                for _ in 0..500_000 {
                    //loop {
                    send_count.fetch_add(1, Ordering::SeqCst);
                    send_fut(
                        mailbox.clone(),
                        Message {
                            ver: 1,
                            priority: p as u32,
                            data: data.clone(),
                        },
                        p,
                        complete_count.clone(),
                        fail_count.clone(),
                    )
                    .await;
                    if sleep_dur > Duration::ZERO {
                        tokio::time::sleep(sleep_dur).await;
                    }
                }
            };

            let mut sends = Vec::new();
            for i in 0..100 {
                sends.push(send(
                    mailbox.clone(),
                    msg.clone(),
                    i,
                    Duration::from_millis(0),
                    complete_count.clone(),
                    send_count.clone(),
                    fail_count.clone(),
                ));
            }
            sends.push(send(
                mailbox.clone(),
                vec![0, 1, 2, 3],
                Priority::MAX,
                Duration::from_millis(5000),
                complete_count.clone(),
                send_count.clone(),
                fail_count.clone(),
            ));

            let stats_fut = async move {
                loop {
                    log::info!(
                        "queue_len: {:?}, completes: {:?}, sends: {:?}, fails: {:?}",
                        mailbox.queue_len(),
                        complete_count.load(Ordering::SeqCst),
                        send_count.load(Ordering::SeqCst),
                        fail_count.load(Ordering::SeqCst),
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            };

            futures::future::join(futures::future::join_all(sends), stats_fut).await;
        };
        send_data_futs.await;
    };

    runner.await;
    Ok(())
}
