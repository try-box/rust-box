use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::{Error, Result};
use collections::PriorityQueue;
use futures::{SinkExt, Stream};
use mpsc::with_priority_channel;
use parking_lot::RwLock;
use tonic::codegen::InterceptedService;
use tonic::metadata::Ascii;
use tonic::service::Interceptor;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic::{metadata::MetadataValue, Request, Status};

use super::transferpb::data_transfer_client::DataTransferClient;
pub use super::transferpb::{self, Message};
use super::{Id, Priority};

type SendError<T> = mpsc::SendError<T>;
type Sender<T> = mpsc::Sender<T, SendError<T>>;

type PriorityQueueType = Arc<parking_lot::RwLock<PriorityQueue<Priority, Message>>>;

type DataTransferClientType = DataTransferClient<InterceptedService<Channel, AuthInterceptor>>;

pub struct ClientBuilder {
    addr: String,
    concurrency_limit: usize,
    connect_timeout: Duration,
    tls: bool,
    tls_ca: Option<String>,
    tls_domain: Option<String>,
    auth_token: Option<String>,
    chunk_size: usize,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            addr: Default::default(),
            concurrency_limit: 10,
            connect_timeout: Duration::from_secs(10),
            tls: false,
            tls_ca: None,
            tls_domain: None,
            auth_token: None,
            chunk_size: CHUNK_SIZE_LIMIT,
        }
    }
}

impl ClientBuilder {
    pub async fn build(self) -> Client {
        Client {
            inner: None,
            builder: Arc::new(self),
        }
    }

    pub async fn connect(self) -> Result<Client, Box<dyn std::error::Error>> {
        let inner = connect(
            self.addr.as_str(),
            self.concurrency_limit,
            self.connect_timeout,
            self.tls,
            self.tls_ca.as_ref(),
            self.tls_domain.as_ref(),
            self.auth_token.clone(),
        )
        .await?;
        Ok(Client {
            inner: Some(inner),
            builder: Arc::new(self),
        })
    }

    pub fn concurrency_limit(mut self, concurrency_limit: usize) -> Self {
        self.concurrency_limit = concurrency_limit;
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }

    pub fn tls(mut self, tls_ca: Option<String>, tls_domain: Option<String>) -> Self {
        self.tls = true;
        self.tls_ca = tls_ca;
        self.tls_domain = tls_domain;
        self
    }

    pub fn auth_token(mut self, token: Option<String>) -> Self {
        self.auth_token = token;
        self
    }

    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
        self
    }
}

#[derive(Clone)]
pub struct Client {
    inner: Option<DataTransferClientType>,
    builder: Arc<ClientBuilder>,
}

impl Client {
    #[inline]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(addr: String) -> ClientBuilder {
        ClientBuilder {
            addr,
            ..Default::default()
        }
    }

    #[inline]
    async fn connect(&mut self) -> Result<&mut DataTransferClientType> {
        if self.inner.is_none() {
            let inner = connect(
                self.builder.addr.as_str(),
                self.builder.concurrency_limit,
                self.builder.connect_timeout,
                self.builder.tls,
                self.builder.tls_ca.as_ref(),
                self.builder.tls_domain.as_ref(),
                self.builder.auth_token.clone(),
            )
            .await?;
            self.inner = Some(inner);
        }
        if let Some(c) = self.inner.as_mut() {
            Ok(c)
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub async fn send(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        self.send_priority(data, Priority::MIN).await
    }

    #[inline]
    pub async fn send_priority(&mut self, data: Vec<u8>, p: Priority) -> Result<Vec<u8>> {
        let chunk_size = self.builder.chunk_size;
        let c = self.connect().await?;
        if data.len() > chunk_size {
            //chunked send
            let mut resp_data = None;
            for msg in split_into_chunks(data.as_slice(), p, chunk_size) {
                let resp = c.send(tonic::Request::new(msg)).await.map_err(Error::new)?;
                resp_data = Some(resp.into_inner().data);
            }
            Ok(resp_data.unwrap_or_default())
        } else {
            let msg = Message {
                id: next_id(),
                priority: p,
                total_chunks: 0,
                chunk_index: 0,
                data,
            };
            let resp = c.send(tonic::Request::new(msg)).await.map_err(Error::new);
            let msg = resp?.into_inner();
            Ok(msg.data)
        }
    }

    #[inline]
    pub async fn transfer_start(&mut self, queue_cap: usize) -> Mailbox {
        let mut this = self.clone();
        let queue = Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
        let (tx, rx) = with_priority_channel(queue.clone(), queue_cap);
        let rx = Receiver::new(rx);
        let mailbox = Mailbox::new(tx, queue, queue_cap, self.builder.chunk_size);

        tokio::spawn(async move {
            loop {
                let c = match this.connect().await {
                    Err(e) => {
                        log::error!("gRPC connect failure, {:?}", e);
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    Ok(c) => c,
                };

                log::trace!("gRPC call transfer ... ");
                if let Err(e) = c.transfer(Request::new(rx.clone())).await {
                    log::error!("gRPC call transfer failure, {:?}", e);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }

                log::info!(
                    "transfer is exit, addr: {:?}, is_closed: {}",
                    this.builder.addr,
                    rx.is_closed()
                );
                break;
            }
        });
        mailbox
    }
}

#[derive(Clone)]
pub struct Mailbox {
    tx: Sender<(Priority, Message)>,
    queue: PriorityQueueType,
    queue_cap: usize,
    chunk_size: usize,
}

impl Mailbox {
    #[inline]
    fn new(
        tx: Sender<(Priority, Message)>,
        queue: PriorityQueueType,
        queue_cap: usize,
        chunk_size: usize,
    ) -> Self {
        Self {
            tx,
            queue,
            queue_cap,
            chunk_size,
        }
    }

    #[inline]
    pub fn queue_len(&self) -> usize {
        self.queue.read().len()
    }

    #[inline]
    pub async fn send(&mut self, data: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.send_priority(data, Priority::MIN).await
    }

    #[inline]
    pub async fn send_priority(
        &mut self,
        data: Vec<u8>,
        p: Priority,
    ) -> Result<(), SendError<Vec<u8>>> {
        if data.len() > self.chunk_size {
            //chunked transfer
            for msg in split_into_chunks(data.as_slice(), p, self.chunk_size) {
                self.tx.send((p, msg)).await.map_err(Self::error)?;
            }
            Ok(())
        } else {
            let msg = Message {
                id: next_id(),
                priority: p,
                total_chunks: 0,
                chunk_index: 0,
                data,
            };
            self.tx.send((p, msg)).await.map_err(Self::error)
        }
    }

    #[inline]
    pub async fn quick_send(&mut self, data: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.send_priority(data, Priority::MAX).await
    }

    #[inline]
    pub fn quick_try_send(&mut self, data: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.try_send_priority(data, Priority::MAX)
    }

    #[inline]
    pub fn try_send(&mut self, data: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.try_send_priority(data, Priority::MIN)
    }

    #[inline]
    pub fn try_send_priority(
        &mut self,
        data: Vec<u8>,
        p: Priority,
    ) -> Result<(), SendError<Vec<u8>>> {
        if self.queue_len() < self.queue_cap {
            if data.len() > self.chunk_size {
                //chunked transfer
                for msg in split_into_chunks(data.as_slice(), p, self.chunk_size) {
                    self.tx.start_send_unpin((p, msg)).map_err(Self::error)?;
                }
                Ok(())
            } else {
                let msg = Message {
                    id: next_id(),
                    priority: p,
                    total_chunks: 0,
                    chunk_index: 0,
                    data,
                };
                self.tx.start_send_unpin((p, msg)).map_err(Self::error)
            }
        } else {
            Err(SendError::<Vec<u8>>::full(data))
        }
    }

    #[inline]
    fn error(e: SendError<(Priority, Message)>) -> SendError<Vec<u8>> {
        if e.is_full() {
            e.into_inner()
                .map(|(_, msg)| SendError::<Vec<u8>>::full(msg.data))
                .unwrap_or_else(|| SendError::<Vec<u8>>::disconnected(None))
        } else if e.is_disconnected() {
            SendError::<Vec<u8>>::disconnected(e.into_inner().map(|(_, msg)| msg.data))
        } else {
            SendError::<Vec<u8>>::disconnected(None)
        }
    }
}

#[derive(Clone)]
struct AuthInterceptor {
    auth_token: Option<MetadataValue<Ascii>>,
}

impl Interceptor for AuthInterceptor {
    #[inline]
    fn call(&mut self, mut request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        if let Some(token) = self.auth_token.clone() {
            request.metadata_mut().insert("authorization", token);
        }
        Ok(request)
    }
}

#[inline]
async fn connect(
    addr: &str,
    concurrency_limit: usize,
    connect_timeout: Duration,
    tls: bool,
    tls_ca: Option<&String>,
    tls_domain: Option<&String>,
    token: Option<String>,
) -> Result<DataTransferClientType> {
    //TLS支持
    let tls_client_cfg = if tls {
        let mut tls_client_cfg = ClientTlsConfig::new();
        if let Some(tls_ca) = tls_ca {
            let pem = std::fs::read_to_string(tls_ca)?;
            tls_client_cfg = tls_client_cfg.ca_certificate(Certificate::from_pem(pem));
        }
        if let Some(tls_domain) = tls_domain {
            tls_client_cfg = tls_client_cfg.domain_name(tls_domain);
        }
        Some(tls_client_cfg)
    } else {
        None
    };

    //gRPC Auth
    let auth_token = if let Some(token) = token {
        if token.is_empty() {
            return Err(Error::msg("auth token is empty"));
        }
        Some(format!("Bearer {}", token).parse::<MetadataValue<_>>()?)
    } else {
        None
    };

    //Concurrency limit
    let concurrency_limit = if concurrency_limit == 0 {
        1
    } else {
        concurrency_limit
    };

    //Endpoint
    let endpoint = Channel::from_shared(format!("http://{}", addr)).map(|endpoint| {
        let endpoint = endpoint.concurrency_limit(concurrency_limit);
        if let Some(tls_client_cfg) = tls_client_cfg {
            endpoint.tls_config(tls_client_cfg)
        } else {
            Ok(endpoint)
        }
    })??;

    //Connect
    let channel = tokio::time::timeout(connect_timeout, endpoint.connect()).await??;

    //Client
    Ok(DataTransferClient::with_interceptor(
        channel,
        AuthInterceptor { auth_token },
    ))
}

#[derive(Clone)]
struct Receiver {
    rx: Arc<RwLock<mpsc::Receiver<(Priority, Message)>>>,
}

impl Receiver {
    fn new(rx: mpsc::Receiver<(Priority, Message)>) -> Self {
        Receiver {
            rx: Arc::new(RwLock::new(rx)),
        }
    }

    #[inline]
    pub fn is_closed(&self) -> bool {
        self.rx.read().is_closed()
    }
}

impl Stream for Receiver {
    type Item = Message;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(self.rx.write().deref_mut()).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some((_, msg))) => Poll::Ready(Some(msg)),
        }
    }
}

#[inline]
pub(crate) fn next_id() -> Id {
    use once_cell::sync::OnceCell;
    use std::sync::atomic::{AtomicU64, Ordering};
    static ID_GENERATOR: OnceCell<AtomicU64> = OnceCell::new();
    let id_generator = ID_GENERATOR.get_or_init(|| AtomicU64::new(1));
    id_generator.fetch_add(1, Ordering::SeqCst)
}

#[inline]
pub(crate) fn split_into_chunks(
    data: &[u8],
    p: Priority,
    chunk_size: usize,
) -> Vec<transferpb::Message> {
    let id = next_id();
    let chunks: Vec<_> = data.chunks(chunk_size).collect();
    let total_chunks = chunks.len() as u32;
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| transferpb::Message {
            id,
            priority: p,
            total_chunks,
            chunk_index: i as u32,
            data: chunk.into(),
        })
        .collect()
}

const CHUNK_SIZE_LIMIT: usize = 1024 * 1024;
