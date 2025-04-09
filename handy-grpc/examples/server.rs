use std::time::Duration;

use futures::StreamExt;
use handy_grpc::Priority;

use handy_grpc::server::{server, Message};
use mpsc::priority_channel as channel;

// cargo run -r --example server --features rate_print

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "server=debug,handy_grpc=debug");
    env_logger::init();

    let laddr = "[::1]:10000".parse().unwrap();
    // let laddr = "0.0.0.0:10001".parse().unwrap();

    let runner = async move {
        let (tx, mut rx) = channel::<Priority, Message>(100_000);

        let recv_data_fut = async move {
            while let Some((_, (data, reply_tx))) = rx.next().await {
                // if data.len() == 1024 * 1024 * 100 {
                //     log::info!("  ==> High priority message, data len 1024 * 1024 * 100",);
                // } else {
                //     log::trace!("  ==> req data len {}", data.len());
                // }
                if let Some(reply_tx) = reply_tx {
                    if let Err(e) = reply_tx.send(Ok(data.len().to_be_bytes().to_vec())) {
                        log::error!("gRPC send result failure, {:?}", e);
                    }
                }
            }
            log::error!("Recv None");
        };

        let run_receiver_fut = async move {
            loop {
                if let Err(e) =
                    server(laddr, tx.clone()).recv_chunks_timeout(Duration::from_secs(30)).run().await
                {
                    log::error!("Run gRPC receiver error, {:?}", e);
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        };
        futures::future::join(recv_data_fut, run_receiver_fut).await;
    };

    runner.await;
    Ok(())
}
