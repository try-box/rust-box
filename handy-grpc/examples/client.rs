use handy_grpc::client::Client;

// cargo run -r --example client

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "client=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    let mut c = Client::new(addr.into()).connect().await?;
    let data = vec![8].repeat(1024 * 1024).repeat(10);
    let send_result = c.send(data).await;
    log::info!("send result({:?})", send_result);

    Ok(())
}
