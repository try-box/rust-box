use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::anyhow;
use collections::PriorityQueue;
use futures::{Stream, StreamExt};
use mpsc::with_priority_channel;
use parking_lot::RwLock;
use scopeguard::defer;
use tokio::sync::oneshot;
use tonic::codegen::InterceptedService;
use tonic::metadata::Ascii;
use tonic::service::Interceptor;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Endpoint};
use tonic::{metadata::MetadataValue, Request, Status};

use super::transferpb::data_transfer_client::DataTransferClient;
pub use super::transferpb::{self, Message};
use super::{
    split_into_chunks, ChunkedBuffer, Error, Id, Priority, Result, CHUNK_SIZE_LIMIT,
    RECV_CHUNKS_TIMEOUT,
};

type Sender<T> = mpsc::Sender<T, mpsc::SendError<T>>;

type PriorityQueueType = Arc<parking_lot::RwLock<PriorityQueue<Priority, Message>>>;

type DataTransferClientType = DataTransferClient<InterceptedService<Channel, AuthInterceptor>>;

type DashMap<K, V> = dashmap::DashMap<K, V, ahash::RandomState>;

pub struct ClientBuilder {
    addr: String,
    concurrency_limit: usize,
    connect_timeout: Option<Duration>,
    timeout: Option<Duration>,
    tls: bool,
    tls_ca: Option<String>,
    tls_domain: Option<String>,
    auth_token: Option<String>,
    chunk_size: usize,
    recv_chunks_timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            addr: Default::default(),
            concurrency_limit: 10,
            connect_timeout: None,
            timeout: None,
            tls: false,
            tls_ca: None,
            tls_domain: None,
            auth_token: None,
            chunk_size: CHUNK_SIZE_LIMIT,
            recv_chunks_timeout: RECV_CHUNKS_TIMEOUT,
        }
    }
}

impl ClientBuilder {
    pub async fn connect(self) -> Result<Client> {
        let inner = connect(
            self.addr.as_str(),
            self.concurrency_limit,
            self.connect_timeout,
            self.timeout,
            self.tls,
            self.tls_ca.as_ref(),
            self.tls_domain.as_ref(),
            self.auth_token.clone(),
        )
        .await?;
        Ok(Client {
            inner,
            builder: Arc::new(self),
        })
    }

    pub fn connect_lazy(self) -> Result<Client> {
        let inner = connect_lazy(
            self.addr.as_str(),
            self.concurrency_limit,
            self.connect_timeout,
            self.timeout,
            self.tls,
            self.tls_ca.as_ref(),
            self.tls_domain.as_ref(),
            self.auth_token.clone(),
        )?;
        Ok(Client {
            inner,
            builder: Arc::new(self),
        })
    }

    pub fn concurrency_limit(mut self, concurrency_limit: usize) -> Self {
        self.concurrency_limit = concurrency_limit;
        self
    }

    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = Some(connect_timeout);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
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

    pub fn recv_chunks_timeout(mut self, recv_chunks_timeout: Duration) -> Self {
        self.recv_chunks_timeout = recv_chunks_timeout;
        self
    }
}

#[derive(Clone)]
pub struct Client {
    inner: DataTransferClientType,
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
    fn connect(&mut self) -> &mut DataTransferClientType {
        &mut self.inner
    }

    //@TODO 使用双向流传输方式替换直接响应试传输数据
    #[inline]
    pub async fn send(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        self.send_priority(data, Priority::MIN).await
    }

    #[inline]
    pub async fn send_priority(&mut self, data: Vec<u8>, p: Priority) -> Result<Vec<u8>> {
        if let Some(t) = self.builder.timeout {
            tokio::time::timeout(t, self._send_priority(data, p)).await?
        } else {
            self._send_priority(data, p).await
        }
    }

    #[inline]
    async fn _send_priority(&mut self, data: Vec<u8>, p: Priority) -> Result<Vec<u8>> {
        let chunk_size = self.builder.chunk_size;
        let timeout = self.builder.timeout;
        let c = self.connect();
        if data.len() > chunk_size {
            //chunked send
            let mut resp_data = None;
            for msg in split_into_chunks(next_id(), data.as_slice(), p, chunk_size) {
                let req = if let Some(t) = timeout {
                    let mut req = tonic::Request::new(msg);
                    req.set_timeout(t);
                    req
                } else {
                    tonic::Request::new(msg)
                };
                let resp = c.send(req).await.map_err(Error::new)?;
                let data = resp.into_inner().data;
                if resp_data.is_none() && data.is_some() {
                    resp_data = data;
                }
            }
            if let Some(resp_data) = resp_data {
                Ok(resp_data)
            } else {
                Err(anyhow!("Timeout"))
            }
        } else {
            let msg = Message {
                id: next_id(),
                priority: p,
                total_chunks: 0,
                chunk_index: 0,
                data: Some(data),
                ..Default::default()
            };
            let req = if let Some(t) = timeout {
                let mut req = tonic::Request::new(msg);
                req.set_timeout(t);
                req
            } else {
                tonic::Request::new(msg)
            };
            let resp = c.send(req).await.map_err(Error::new);
            let msg = resp?.into_inner();
            Ok(msg.data.unwrap_or_default())
        }
    }

    #[inline]
    pub async fn transfer_start(&mut self, req_queue_cap: usize) -> Mailbox {
        let mut this = self.clone();
        let req_queue = Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
        let (tx, rx) = with_priority_channel(req_queue.clone(), req_queue_cap);
        let rx = Receiver::new(rx);
        let mailbox = Mailbox::new(tx, req_queue, req_queue_cap, self.builder.chunk_size);
        let addr = self.builder.addr.clone();
        tokio::spawn(async move {
            loop {
                log::trace!("gRPC call transfer ... ");
                if let Err(e) = this.connect().transfer(Request::new(rx.clone())).await {
                    log::warn!(
                        "gRPC call transfer failure, addr:{}, {}",
                        addr,
                        e.to_string()
                    );
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

    #[inline]
    pub async fn duplex_transfer_start(&mut self, queue_cap: usize) -> DuplexMailbox {
        let mut this = self.clone();
        let req_queue = Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
        let (req_tx, req_rx) = with_priority_channel(req_queue.clone(), queue_cap);
        let req_rx = Receiver::new(req_rx);

        let resp_queue = Arc::new(parking_lot::RwLock::new(PriorityQueue::default()));
        let (mut resp_tx, resp_rx) = with_priority_channel(resp_queue.clone(), queue_cap);
        let resp_rx = Receiver::new(resp_rx);

        let mailbox = DuplexMailbox::new(
            req_tx,
            resp_rx,
            req_queue,
            resp_queue,
            queue_cap,
            self.builder.chunk_size,
            self.builder.recv_chunks_timeout,
            self.builder.timeout,
        );
        let addr = self.builder.addr.clone();
        tokio::spawn(async move {
            'outer: loop {
                log::trace!("gRPC call duplex transfer ... ");
                match this
                    .connect()
                    .duplex_transfer(Request::new(req_rx.clone()))
                    .await
                {
                    Err(e) => {
                        log::warn!(
                            "gRPC call duplex transfer failure, addr:{}, {}",
                            addr,
                            e.to_string()
                        );
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    Ok(resp) => {
                        let mut resp_stream = resp.into_inner();
                        while let Some(received) = resp_stream.next().await {
                            match received {
                                Err(e) => {
                                    log::warn!(
                                        "gRPC duplex transfer response stream recv failure, addr:{}, {}",
                                        addr,
                                        e.to_string()
                                    );
                                    tokio::time::sleep(Duration::from_secs(3)).await;
                                    continue 'outer;
                                }
                                Ok(received) => {
                                    if let Err(e) =
                                        resp_tx.send((received.priority, received)).await
                                    {
                                        log::warn!(
                                            "gRPC duplex transfer send response message failure, addr:{}, {}",
                                            addr,
                                            e.to_string()
                                        );
                                    }
                                }
                            }
                        }
                        log::warn!(
                            "gRPC duplex transfer response stream recv None, addr:{}",
                            addr,
                        );
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                }
            }
        });
        mailbox
    }
}

#[derive(Clone)]
pub struct Mailbox {
    req_tx: Sender<(Priority, Message)>,
    req_queue: PriorityQueueType,
    req_queue_cap: usize,
    chunk_size: usize,
}

impl Mailbox {
    #[inline]
    fn new(
        req_tx: Sender<(Priority, Message)>,
        req_queue: PriorityQueueType,
        req_queue_cap: usize,
        chunk_size: usize,
    ) -> Self {
        Self {
            req_tx,
            req_queue,
            req_queue_cap,
            chunk_size,
        }
    }

    #[inline]
    pub fn req_queue_is_full(&self) -> bool {
        self.req_queue_len() >= self.req_queue_cap
    }

    #[inline]
    pub fn req_queue_len(&self) -> usize {
        self.req_queue.read().len()
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
            for msg in split_into_chunks(next_id(), data.as_slice(), p, self.chunk_size) {
                self.req_tx.send((p, msg)).await.map_err(Self::error)?;
            }
            Ok(())
        } else {
            let msg = Message {
                id: next_id(),
                priority: p,
                total_chunks: 0,
                chunk_index: 0,
                data: Some(data),
                ..Default::default()
            };
            self.req_tx.send((p, msg)).await.map_err(Self::error)
        }
    }

    #[inline]
    pub async fn quick_send(&mut self, data: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.send_priority(data, Priority::MAX).await
    }

    #[inline]
    fn error(e: mpsc::SendError<(Priority, Message)>) -> SendError<Vec<u8>> {
        if e.is_full() {
            e.into_inner()
                .map(|(_, msg)| SendError::<Vec<u8>>::full(msg.data.unwrap_or_default()))
                .unwrap_or_else(|| SendError::<Vec<u8>>::disconnected(None))
        } else if e.is_disconnected() {
            SendError::<Vec<u8>>::disconnected(
                e.into_inner().map(|(_, msg)| msg.data.unwrap_or_default()),
            )
        } else {
            SendError::<Vec<u8>>::disconnected(None)
        }
    }
}

#[derive(Clone)]
pub struct DuplexMailbox {
    req_tx: Sender<(Priority, Message)>,
    req_queue: PriorityQueueType,
    resp_queue: PriorityQueueType,
    resp_senders: Arc<DashMap<Id, oneshot::Sender<Result<Vec<u8>>>>>,
    queue_cap: usize,
    chunk_size: usize,
    timeout: Option<Duration>,
}

#[allow(clippy::too_many_arguments)]
impl DuplexMailbox {
    #[inline]
    fn new(
        req_tx: Sender<(Priority, Message)>,
        resp_rx: Receiver,
        req_queue: PriorityQueueType,
        resp_queue: PriorityQueueType,
        queue_cap: usize,
        chunk_size: usize,
        recv_chunks_timeout: Duration,
        timeout: Option<Duration>,
    ) -> Self {
        let resp_chunked_buffer = ChunkedBuffer::new(recv_chunks_timeout);
        Self {
            req_tx,
            req_queue,
            resp_queue,
            resp_senders: Arc::new(DashMap::default()),
            queue_cap,
            chunk_size,
            timeout,
        }
        .start(resp_rx, resp_chunked_buffer)
    }

    fn start(self, mut resp_rx: Receiver, resp_chunked_buffer: ChunkedBuffer) -> Self {
        let resp_senders = self.resp_senders.clone();
        tokio::spawn(async move {
            let mut removed_ids = Vec::new();
            while let Some(mut msg) = resp_rx.next().await {
                if let Some(err) = msg.err.take() {
                    if let Some((_, resp_sender)) = resp_senders.remove(&msg.id) {
                        if !resp_sender.is_closed() {
                            if let Err(e) = resp_sender.send(Err(anyhow!(err))) {
                                log::warn!("response sender send fail, {:?}", e);
                            }
                        } else {
                            log::warn!("response sender is closed");
                        }
                    }
                    continue;
                }

                let msg = resp_chunked_buffer
                    .merge(msg, None, Some(&mut removed_ids))
                    .await;

                if !removed_ids.is_empty() {
                    for removed_id in removed_ids.drain(..) {
                        log::debug!("removed_id: {}", removed_id);
                        resp_senders.remove(&removed_id);
                    }
                }

                let (id, data) = if let Some((id, _, data)) = msg {
                    (id, data)
                } else {
                    continue;
                };

                if let Some((_, resp_sender)) = resp_senders.remove(&id) {
                    if !resp_sender.is_closed() {
                        if let Err(e) = resp_sender.send(Ok(data)) {
                            log::warn!("response sender send fail, {:?}", e);
                        }
                    } else {
                        log::warn!("response sender is closed");
                    }
                }
            }
            log::info!("exit response Receiver");
        });
        self
    }

    #[inline]
    pub fn req_queue_is_full(&self) -> bool {
        self.req_queue_len() >= self.queue_cap
    }

    #[inline]
    pub fn req_queue_len(&self) -> usize {
        self.req_queue.read().len()
    }

    #[inline]
    pub fn resp_queue_len(&self) -> usize {
        self.resp_queue.read().len()
    }

    #[inline]
    pub fn resp_senders_len(&self) -> usize {
        self.resp_senders.len()
    }

    #[inline]
    pub async fn send(&mut self, data: Vec<u8>) -> Result<Vec<u8>, SendError<Option<Vec<u8>>>> {
        self.send_priority(data, Priority::MIN).await
    }

    #[inline]
    pub async fn send_priority(
        &mut self,
        data: Vec<u8>,
        p: Priority,
    ) -> Result<Vec<u8>, SendError<Option<Vec<u8>>>> {
        let (res_tx, res_rx) = oneshot::channel::<Result<Vec<u8>>>();
        let id = next_id();
        let resp_senders = self.resp_senders.clone();
        resp_senders.insert(id, res_tx);
        defer! {
            resp_senders.remove(&id);
        }
        self._send_priority(id, data, p, res_rx).await
    }
    #[inline]
    async fn _send_priority(
        &mut self,
        id: Id,
        data: Vec<u8>,
        p: Priority,
        res_rx: oneshot::Receiver<Result<Vec<u8>>>,
    ) -> Result<Vec<u8>, SendError<Option<Vec<u8>>>> {
        if data.len() > self.chunk_size {
            //chunked transfer
            for msg in split_into_chunks(id, data.as_slice(), p, self.chunk_size) {
                self.req_tx.send((p, msg)).await.map_err(Self::error)?;
            }
        } else {
            let msg = Message {
                id,
                priority: p,
                total_chunks: 0,
                chunk_index: 0,
                data: Some(data),
                ..Default::default()
            };
            self.req_tx.send((p, msg)).await.map_err(Self::error)?;
        }

        let res = tokio::time::timeout(
            self.timeout.unwrap_or_else(|| Duration::from_secs(120)),
            res_rx,
        )
        .await
        .map_err(|e| SendError::error(e.to_string(), None))?
        .map_err(|e| SendError::error(e.to_string(), None))?
        .map_err(|e| SendError::error(e.to_string(), None))?;

        Ok(res)
    }

    #[inline]
    pub async fn quick_send(
        &mut self,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, SendError<Option<Vec<u8>>>> {
        self.send_priority(data, Priority::MAX).await
    }

    #[inline]
    fn error(e: mpsc::SendError<(Priority, Message)>) -> SendError<Option<Vec<u8>>> {
        if e.is_full() {
            e.into_inner()
                .map(|(_, msg)| SendError::<Option<Vec<u8>>>::full(msg.data))
                .unwrap_or_else(|| SendError::<Option<Vec<u8>>>::disconnected(None))
        } else if e.is_disconnected() {
            SendError::<Option<Vec<u8>>>::disconnected(e.into_inner().map(|(_, msg)| msg.data))
        } else {
            SendError::<Option<Vec<u8>>>::disconnected(None)
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

#[allow(clippy::too_many_arguments)]
#[inline]
async fn connect(
    addr: &str,
    concurrency_limit: usize,
    connect_timeout: Option<Duration>,
    timeout: Option<Duration>,
    tls: bool,
    tls_ca: Option<&String>,
    tls_domain: Option<&String>,
    token: Option<String>,
) -> Result<DataTransferClientType> {
    let (endpoint, interceptor) = build_endpoint(
        addr,
        concurrency_limit,
        connect_timeout,
        timeout,
        tls,
        tls_ca,
        tls_domain,
        token,
    )?;

    //Connect
    let channel = endpoint.connect().await?;

    //Client
    Ok(DataTransferClient::with_interceptor(channel, interceptor))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn connect_lazy(
    addr: &str,
    concurrency_limit: usize,
    connect_timeout: Option<Duration>,
    timeout: Option<Duration>,
    tls: bool,
    tls_ca: Option<&String>,
    tls_domain: Option<&String>,
    token: Option<String>,
) -> Result<DataTransferClientType> {
    let (endpoint, interceptor) = build_endpoint(
        addr,
        concurrency_limit,
        connect_timeout,
        timeout,
        tls,
        tls_ca,
        tls_domain,
        token,
    )?;

    //Connect lazy
    let channel = endpoint.connect_lazy();

    //Client
    Ok(DataTransferClient::with_interceptor(channel, interceptor))
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn build_endpoint(
    addr: &str,
    concurrency_limit: usize,
    connect_timeout: Option<Duration>,
    timeout: Option<Duration>,
    tls: bool,
    tls_ca: Option<&String>,
    tls_domain: Option<&String>,
    token: Option<String>,
) -> Result<(Endpoint, AuthInterceptor)> {
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
        let mut endpoint = endpoint.concurrency_limit(concurrency_limit);
        if let Some(connect_timeout) = connect_timeout {
            endpoint = endpoint.connect_timeout(connect_timeout);
        }
        if let Some(timeout) = timeout {
            endpoint = endpoint.timeout(timeout);
        }
        if let Some(tls_client_cfg) = tls_client_cfg {
            endpoint.tls_config(tls_client_cfg)
        } else {
            Ok(endpoint)
        }
    })??;
    Ok((endpoint, AuthInterceptor { auth_token }))
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

#[derive(Clone, PartialEq, Eq)]
pub enum SendError<T> {
    SendError(mpsc::SendError<T>),
    Error(String, Option<T>),
}

impl<T> SendError<T> {
    #[inline]
    pub fn full(val: T) -> Self {
        SendError::SendError(mpsc::SendError::full(val))
    }

    #[inline]
    pub fn disconnected(val: Option<T>) -> Self {
        SendError::SendError(mpsc::SendError::disconnected(val))
    }

    #[inline]
    pub fn error(e: String, val: Option<T>) -> Self {
        SendError::Error(e, val)
    }
}

impl<T> core::fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SendError::SendError(e) => {
                write!(f, "{:?}", e)
            }
            SendError::Error(e, _) => {
                write!(f, "{:?}", e)
            }
        }
    }
}

impl<T> core::fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SendError::SendError(e) => f.debug_struct("SendError").field("reason", e).finish(),
            SendError::Error(e, _) => f.debug_struct("SendError").field("reason", e).finish(),
        }
    }
}

impl<T> SendError<T> {
    /// Returns `true` if this error is a result of the mpsc being full.
    #[inline]
    pub fn is_full(&self) -> bool {
        if let SendError::SendError(e) = self {
            e.is_full()
        } else {
            false
        }
    }

    /// Returns `true` if this error is a result of the receiver being dropped.
    #[inline]
    pub fn is_disconnected(&self) -> bool {
        if let SendError::SendError(e) = self {
            e.is_disconnected()
        } else {
            false
        }
    }

    /// Returns the message that was attempted to be sent but failed.
    #[inline]
    pub fn into_inner(self) -> Option<T> {
        match self {
            SendError::SendError(inner) => inner.into_inner(),
            SendError::Error(_, val) => val,
        }
    }
}

impl<T: core::any::Any> std::error::Error for SendError<T> {}
