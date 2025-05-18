use handy_grpc::client::{Client, DuplexMailbox};
use prost::bytes::BufMut;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

// cargo run -r --example bench_duplex_sender

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "bench_duplex_sender=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    let concurrency_sends = 256;

    let mut client = Client::new(addr.into())
        .chunk_size(1024 * 1024)
        .timeout(Duration::from_millis(30000))
        .connect_lazy()?;

    let mut senders = Vec::new();
    for i in 0..1 {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter1 = counter.clone();
        let duplex_mailbox = client.duplex_transfer_start(10_000).await;
        let duplex_mailbox1 = duplex_mailbox.clone();
        let runner = async move {
            let timeouts = Arc::new(AtomicUsize::new(0));
            let timeouts1 = timeouts.clone();
            let send_data_futs = async move {
                let send = |mut mailbox: DuplexMailbox,
                            n: usize,
                            timeouts: Arc<AtomicUsize>,
                            c: Arc<AtomicUsize>| async move {
                    let timeouts = timeouts.clone();
                    for _ in 0..n {
                        c.fetch_add(1, Ordering::SeqCst);
                        // let mut data = vec![8].repeat(1024 * 1024).repeat(100);
                        let mut data = vec![8].repeat(1024 * 1);
                        data.put_u64(rand::random::<u64>());
                        let now = std::time::Instant::now();
                        let _send_result = match mailbox.send(data).await {
                            Ok(send_result) => send_result,
                            Err(e) => {
                                log::warn!("send error({:?})", e);
                                timeouts.fetch_add(1, Ordering::SeqCst);
                                continue;
                            }
                        };
                        if now.elapsed().as_millis() > 10000 {
                            log::info!("call cost time: {:?}", now.elapsed());
                        }
                        // if data != send_result {
                        //     log::warn!("send result({:?})", send_result.len());
                        // }
                    }
                    log::info!("exit ...");
                };

                let mut sends = Vec::new();
                for _ in 0..concurrency_sends {
                    sends.push(send(
                        duplex_mailbox.clone(),
                        5_000_000_000,
                        timeouts1.clone(),
                        counter.clone(),
                    ));
                }
                futures::future::join_all(sends).await;
            };

            let stats = async {
                loop {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    log::info!(
                    "{} req_queue_len: {}, resp_queue_len: {}, resp_senders_len: {}, counter: {}, timeouts: {}", i,
                    duplex_mailbox1.req_queue_len(),
                    duplex_mailbox1.resp_queue_len(),
                    duplex_mailbox1.resp_senders_len(),
                    counter1.load(Ordering::SeqCst),
                    timeouts.load(Ordering::SeqCst)
                );
                }
            };
            // send_data_futs.await;
            futures::future::join(stats, send_data_futs).await;
            log::info!("timeouts: {}", timeouts.load(Ordering::SeqCst));
        };
        // runner.await;
        senders.push(tokio::spawn(runner));
    }
    futures::future::join_all(senders).await;
    Ok(())
}
