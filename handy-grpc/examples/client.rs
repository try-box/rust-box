use handy_grpc::client::{Client, Message};

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
                    let send_result = c
                        .send(Message {
                            ver: 1,
                            priority: 0,
                            data: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                        })
                        .await;
                    log::trace!("send_result, {:?}", send_result);
                }
            };

            let mut sends = Vec::new();
            for _ in 0..32 {
                sends.push(send(client.clone()));
            }
            futures::future::join_all(sends).await;
        };
        send_data_futs.await;
    };

    runner.await;
    Ok(())
}
