use handy_grpc::client::Client;

// cargo run -r --example client

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "client=debug,handy_grpc=debug");
    env_logger::init();

    let addr = "[::1]:10000";

    let runner = async move {
        let client = Client::new(addr.into()).connect().await.unwrap();
        let send_data_futs = async move {
            let send = |mut c: Client| async move {
                loop {
                    let data = vec![8].repeat(1024 * 1024).repeat(100);
                    log::info!("send data({}): {:?} ... ", data.len(), &data[0..20]);
                    let send_result = c.send(data).await.unwrap();
                    log::info!(
                        "send result({:?})",
                        usize::from_be_bytes(send_result.as_slice().try_into().unwrap())
                    );
                    break;
                }
            };

            let mut sends = Vec::new();
            for _ in 0..1 {
                sends.push(send(client.clone()));
            }
            futures::future::join_all(sends).await;
        };
        send_data_futs.await;
    };

    runner.await;
    Ok(())
}
