use handy_grpc::client::Client;
use std::time::Duration;

// cargo run -r --example sender

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "sender=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    let mut mailbox = Client::new(addr.into())
        .build()
        .await
        .transfer_start(10_000)
        .await;

    let send_result = mailbox.send(vec![8].repeat(1024 * 1024)).await;
    log::info!("send result({:?})", send_result);
    while mailbox.queue_len() > 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(())
}
