use std::time::Duration;

use futures::channel::mpsc::channel;
use futures::StreamExt;

use handy_grpc::server::{run, Message};

// cargo run -r --example receiver --features rate_print

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "receiver=debug,handy_grpc=debug");
    env_logger::init();

    let addr = "[::1]:10000".parse().unwrap();

    let runner = async move {
        let (tx, mut rx) = channel::<Message>(100_000);

        let recv_data_fut = async move {
            while let Some((msg, reply_tx)) = rx.next().await {
                if msg.data.len() == 4 {
                    log::info!("  ==> High priority message, data len 4",);
                } else {
                    log::trace!("  ==> req = ver: {}, data len {}", msg.ver, msg.data.len());
                }
                if let Some(reply_tx) = reply_tx {
                    if let Err(e) = reply_tx.send(Ok(msg)) {
                        log::error!("gRPC send result failure, {:?}", e);
                    }
                }
                //tokio::time::sleep(Duration::from_nanos(1)).await;
            }
            log::error!("Recv None");
        };

        let run_receiver_fut = async move {
            loop {
                if let Err(e) = run(addr, tx.clone(), None, None).await {
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
