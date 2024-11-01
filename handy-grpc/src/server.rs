use std::cmp::Reverse;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures::channel::oneshot;
use futures::StreamExt;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::transport::{self, Identity, ServerTlsConfig};
use tonic::{Request, Response, Status};

use anyhow::{Error, Result};
use collections::PriorityQueue;
use dequemap::DequeBTreeMap;
use tokio::sync::RwLock;

#[cfg(feature = "rate")]
use rate::Counter;

use super::transferpb::data_transfer_server::{DataTransfer, DataTransferServer};
use super::transferpb::{self, Empty};
use super::{Id, Priority};

type TX = mpsc::Sender<(Priority, Message), mpsc::SendError<(Priority, Message)>>;

pub type Message = (Vec<u8>, Option<oneshot::Sender<Result<Vec<u8>>>>);

pub struct Server {
    laddr: SocketAddr,
    tx: TX,
    tls: Option<TLS>,
    token: Option<String>,
    recv_chunks_timeout: Duration,
    reuseaddr: bool,
    reuseport: bool,
}

pub fn server(laddr: SocketAddr, tx: TX) -> Server {
    Server {
        laddr,
        tx,
        tls: None,
        token: None,
        recv_chunks_timeout: RECV_CHUNKS_TIMEOUT,
        reuseaddr: true,
        reuseport: false,
    }
}

impl Server {
    pub fn tls(mut self, tls: TLS) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }

    pub fn recv_chunks_timeout(mut self, recv_chunks_timeout: Duration) -> Self {
        self.recv_chunks_timeout = recv_chunks_timeout;
        self
    }

    pub fn reuseaddr(mut self, reuseaddr: bool) -> Self {
        self.reuseaddr = reuseaddr;
        self
    }

    pub fn reuseport(mut self, reuseport: bool) -> Self {
        self.reuseport = reuseport;
        self
    }

    pub async fn run(self) -> Result<()> {
        let mut builder = transport::Server::builder();

        //Check for TLS and generate an identity.
        let (tls_identity, protocol) = if let Some(tls) = self.tls {
            let cert = std::fs::read_to_string(tls.server_cert)?;
            let key = std::fs::read_to_string(tls.server_key)?;
            (Some(Identity::from_pem(cert, key)), "tls")
        } else {
            (None, "tcp")
        };

        //Configure TLS.
        if let Some(tls_identity) = tls_identity {
            builder = builder
                .tls_config(ServerTlsConfig::new().identity(tls_identity))
                .map_err(Error::new)?;
        }

        //Check if token validation is required and create the service.
        let auth_token = if let Some(token) = self.token {
            if token.is_empty() {
                return Err(Error::msg("auth token is empty"));
            }
            let token =
                MetadataValue::try_from(&format!("Bearer {}", token)).map_err(Error::new)?;
            Some(token)
        } else {
            None
        };
        let service = DataTransferServer::with_interceptor(
            DataTransferService::new(self.tx, self.recv_chunks_timeout),
            AuthInterceptor { auth_token },
        );

        log::info!(
            "gRPC DataTransfer is listening on {}://{:?}",
            protocol,
            self.laddr
        );

        let server = builder.add_service(service);

        #[cfg(any(feature = "reuseport", feature = "reuseaddr"))]
        #[cfg(all(feature = "socket2", feature = "tokio-stream"))]
        {
            let listener = socket2_bind(self.laddr, 1024, self.reuseaddr, self.reuseport)?;
            server.serve_with_incoming(listener).await?;
        }
        #[cfg(not(any(feature = "reuseport", feature = "reuseaddr")))]
        server.serve(self.laddr).await?;

        Ok(())
    }
}

pub struct DataTransferService {
    #[cfg(feature = "rate")]
    counter: Counter,
    tx: TX,
    chunked_buffer: ChunkedBuffer,
}

impl DataTransferService {
    pub fn new(tx: TX, recv_chunks_timeout: Duration) -> Self {
        #[cfg(feature = "rate")]
        let counter = Counter::new(std::time::Duration::from_secs(5));
        #[cfg(feature = "rate_print")]
        {
            let c = counter.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    log::info!("total: {}, rate: {:?}", c.total(), c.rate());
                }
            });
        }
        Self {
            #[cfg(feature = "rate")]
            counter,
            tx,
            chunked_buffer: ChunkedBuffer::new(recv_chunks_timeout),
        }
    }

    #[inline]
    fn chunk_empty_result() -> Response<transferpb::Message> {
        let resp = transferpb::Message {
            id: 0,
            priority: 0,
            total_chunks: 0,
            chunk_index: 0,
            data: Vec::new(),
        };
        Response::new(resp)
    }
}

#[tonic::async_trait]
impl DataTransfer for DataTransferService {
    #[inline]
    async fn transfer(
        &self,
        request: Request<tonic::Streaming<transferpb::Message>>,
    ) -> Result<Response<Empty>, Status> {
        let remote_addr = request.remote_addr();
        let mut tx = self.tx.clone();
        let mut stream = request.into_inner();
        while let Some(req) = stream.next().await {
            log::trace!("Request: {:?}", req);
            let req = req?;

            let (priority, data) =
                if let Some((priority, data)) = self.chunked_buffer.merge(req, remote_addr).await {
                    (priority, data)
                } else {
                    continue;
                };

            #[cfg(feature = "rate")]
            self.counter.inc();

            tx.send((priority, (data, None)))
                .await
                .map_err(|e| Status::cancelled(e.to_string()))?;
        }
        log::trace!("Response Empty");
        Ok(Response::new(Empty {}))
    }

    #[inline]
    async fn send(
        &self,
        request: Request<transferpb::Message>,
    ) -> Result<Response<transferpb::Message>, Status> {
        let remote_addr = request.remote_addr();
        let req = request.into_inner();
        log::trace!("Request: {:?}", req);

        let (priority, data) =
            if let Some((priority, data)) = self.chunked_buffer.merge(req, remote_addr).await {
                (priority, data)
            } else {
                return Ok(Self::chunk_empty_result());
            };

        #[cfg(feature = "rate")]
        self.counter.inc();

        let mut tx = self.tx.clone();
        let (res_tx, res_rx) = oneshot::channel();
        tx.send((priority, (data, Some(res_tx))))
            .await
            .map_err(|e| Status::cancelled(e.to_string()))?;

        let res = res_rx
            .await
            .map_err(|e| Status::cancelled(e.to_string()))?
            .map_err(|e| Status::internal(e.to_string()))?;

        let resp = transferpb::Message {
            id: 0,
            priority: 0,
            total_chunks: 0,
            chunk_index: 0,
            data: res,
        };
        Ok(Response::new(resp))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TLS {
    #[serde(default)]
    pub server_cert: String,
    #[serde(default)]
    pub server_key: String,
    pub client_ca: Option<String>,
    pub client_domain: Option<String>,
}

#[derive(Clone)]
struct AuthInterceptor {
    auth_token: Option<MetadataValue<Ascii>>,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, Status> {
        if let Some(token) = self.auth_token.clone() {
            match request.metadata().get("authorization") {
                Some(t) if token == t => Ok(request),
                _ => Err(tonic::Status::unauthenticated("No valid auth token")),
            }
        } else {
            Ok(request)
        }
    }
}

#[inline]
#[cfg(all(feature = "socket2", feature = "tokio-stream"))]
fn socket2_bind(
    laddr: std::net::SocketAddr,
    backlog: i32,
    _reuseaddr: bool,
    _reuseport: bool,
) -> anyhow::Result<tokio_stream::wrappers::TcpListenerStream> {
    use socket2::{Domain, SockAddr, Socket, Type};
    let builder = Socket::new(Domain::for_address(laddr), Type::STREAM, None)?;
    builder.set_nonblocking(true)?;
    #[cfg(unix)]
    #[cfg(feature = "reuseaddr")]
    builder.set_reuse_address(_reuseaddr)?;
    #[cfg(unix)]
    #[cfg(feature = "reuseport")]
    builder.set_reuse_port(_reuseport)?;
    builder.bind(&SockAddr::from(laddr))?;
    builder.listen(backlog)?;
    let listener = tokio_stream::wrappers::TcpListenerStream::new(
        tokio::net::TcpListener::from_std(std::net::TcpListener::from(builder))?,
    );
    Ok(listener)
}

#[allow(clippy::type_complexity)]
struct ChunkedBuffer {
    data_buffs: RwLock<
        DequeBTreeMap<
            (Option<SocketAddr>, Id),
            (Instant, PriorityQueue<Reverse<u32>, transferpb::Message>),
        >,
    >,
    recv_chunks_timeout: Duration,
}

impl ChunkedBuffer {
    fn new(recv_chunks_timeout: Duration) -> Self {
        ChunkedBuffer {
            data_buffs: RwLock::new(DequeBTreeMap::default()),
            recv_chunks_timeout,
        }
    }

    #[inline]
    async fn merge(
        &self,
        req: transferpb::Message,
        remote_addr: Option<SocketAddr>,
    ) -> Option<(Priority, Vec<u8>)> {
        if req.total_chunks > 1 {
            let mut data_buffs = self.data_buffs.write().await;
            while let Some(true) = data_buffs
                .front()
                .map(|(_, (t, q))| q.is_empty() || t.elapsed() > self.recv_chunks_timeout)
            {
                data_buffs.pop_front();
            }
            let (_, data_buff) = data_buffs
                .entry((remote_addr, req.id))
                .or_insert_with(|| (Instant::now(), PriorityQueue::default()));
            let total_chunks = req.total_chunks;
            let priority = req.priority;
            data_buff.push(Reverse(req.chunk_index), req);

            if data_buff.len() >= total_chunks as usize {
                let mut chunks = Vec::with_capacity(data_buff.len());
                while let Some((_, msg)) = data_buff.pop() {
                    chunks.push(msg.data);
                }
                Some((priority, chunks.into_iter().flatten().collect()))
            } else {
                None
            }
        } else {
            Some((req.priority, req.data))
        }
    }
}

//Receive chunk data timeout
const RECV_CHUNKS_TIMEOUT: Duration = Duration::from_secs(15);
