use handy_grpc::client::Client;
use std::time::Duration;

// cargo run -r --example sender

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "sender=info,handy_grpc=info");
    env_logger::init();

    let addr = "[::1]:10000";

    let mut client = Client::new(addr.into()).chunk_size(10).connect_lazy()?;

    let mut mailbox = client.transfer_start(10_000).await;
    let mut duplex_mailbox = client.duplex_transfer_start(10_000).await;

    let send_result = mailbox.send(vec![8].repeat(1024 * 1)).await;
    log::info!("send result({:?})", send_result);

    // let mut data: Vec<u8> = vec![8].repeat(1024 * 1024).repeat(2);
    let mut data: Vec<u8> = [
        vec![6].repeat(10),
        vec![7].repeat(10),
        vec![8].repeat(10),
        // vec![9].repeat(10),
    ]
    .concat()
    .to_vec();
    data.push(0x3);
    // let mut data = vec![8].repeat(1024);
    let random_head = rand::random::<u64>().to_be_bytes();
    let random_tail = rand::random::<u64>().to_be_bytes();
    log::debug!(
        "data len: {}, random_head: {:?}, random_tail: {:?}",
        data.len(),
        random_head,
        random_tail
    );
    let data_len = data.len();
    let data = [
        random_head.as_slice(),
        data.as_slice(),
        random_tail.as_slice(),
    ]
    .concat()
    .to_vec();

    log::debug!(
        "random len: {} {} {}",
        data.len(),
        data_len,
        data.len() - data_len
    );
    let data_len = data.len();
    let duplex_send_result = duplex_mailbox.send(data).await?;
    let head_8: &[u8] = &duplex_send_result[0..8];
    let last_8: &[u8] = &duplex_send_result[duplex_send_result.len() - 8..];
    log::debug!("duplex_send_result: {:?}", &duplex_send_result);
    log::info!(
        "duplex send result({:?}), head_8: {:?}, last_8: {:?}",
        duplex_send_result.len(),
        head_8,
        last_8
    );
    assert_eq!(data_len, duplex_send_result.len());
    assert_eq!(random_head, head_8);
    assert_eq!(random_tail, last_8);

    // while mailbox.queue_len() > 0 {
    tokio::time::sleep(Duration::from_secs(1)).await;
    // }
    Ok(())
}
