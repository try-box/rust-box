use handy_grpc::client::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// cargo run -r --example bench_client

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "bench_client=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    let concurrency_limit = 128;

    let runner = async move {
        let client = Client::new(addr.into())
            .concurrency_limit(concurrency_limit)
            .chunk_size(1024 * 1024 * 2)
            .connect_lazy()
            .unwrap();
        let timeouts = Arc::new(AtomicUsize::new(0));
        let timeouts1 = timeouts.clone();
        let send_data_futs = async move {
            let send = |mut c: Client, n: usize, timeouts: Arc<AtomicUsize>| async move {
                let timeouts = timeouts.clone();
                for _ in 0..n {
                    // let data = vec![8].repeat(1024 * 1024).repeat(100);
                    let data = vec![8].repeat(1024 * 10);
                    let send_result = match c.send(data).await {
                        Ok(send_result) => send_result,
                        Err(e) => {
                            log::warn!("send error({:?})", e);
                            timeouts.fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                    };
                    match send_result.as_slice().try_into() {
                        Ok(res) => {
                            log::debug!("send result({:?})", usize::from_be_bytes(res));
                        }
                        Err(_) => {
                            log::info!("send result({:?})", send_result);
                        }
                    }
                    // break;
                }
            };

            let mut sends = Vec::new();
            for _ in 0..(concurrency_limit * 10) {
                sends.push(send(client.clone(), 5_000_000, timeouts1.clone()));
            }
            futures::future::join_all(sends).await;
        };
        send_data_futs.await;
        log::info!("timeouts: {}", timeouts.load(Ordering::SeqCst));
    };

    runner.await;
    Ok(())
}
