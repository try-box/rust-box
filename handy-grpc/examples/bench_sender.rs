use handy_grpc::client::Client;
use std::time::Duration;

// cargo run -r --example bench_sender

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "bench_sender=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    // let mut c = Client::new(addr.into()).connect().await?;
    let mut c = Client::new(addr.into()).connect_lazy()?;

    let send_result = c.send(vec![1, 2, 3, 4, 5]).await;
    log::info!("send result({:?})", send_result);

    let mut mailbox = c.transfer_start(10_000).await;

    let _ = tokio::spawn(async move {
        for _ in 0..50_000_000 {
            let send_result = mailbox.send(vec![8].repeat(1024)).await;
            if send_result.is_err() {
                log::info!("send result({:?})", send_result);
            }
        }

        while mailbox.queue_len() > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await;

    Ok(())
}
