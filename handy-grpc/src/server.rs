use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use futures::channel::oneshot;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::{interceptor::InterceptedService, Interceptor};
use tonic::transport::{self, Identity, ServerTlsConfig};
use tonic::{Request, Response, Status};

use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};

use anyhow::{Error, Result};

#[cfg(feature = "rate")]
use rate::Counter;

use super::transferpb::data_transfer_server::{DataTransfer, DataTransferServer};
use super::transferpb::{self, Empty};
use super::{
    split_into_chunks, ChunkedBuffer, Id, Priority, CHUNK_SIZE_LIMIT, RECV_CHUNKS_TIMEOUT,
};

type ResponseStream = Pin<Box<dyn Stream<Item = Result<transferpb::Message, Status>> + Send>>;
type SendResult<T> = Result<Response<T>, Status>;

type TX = mpsc::Sender<(Priority, Message), mpsc::SendError<(Priority, Message)>>;

pub type Message = (Vec<u8>, Option<oneshot::Sender<Result<Vec<u8>>>>);

pub struct Server {
    laddr: SocketAddr,
    tx: TX,
    tls: Option<TLS>,
    token: Option<String>,
    recv_chunks_timeout: Duration,
    max_decoding_message_size: Option<usize>,
    max_encoding_message_size: Option<usize>,
    reuseaddr: bool,
    reuseport: bool,
    chunk_size: usize,
}

pub fn server(laddr: SocketAddr, tx: TX) -> Server {
    Server {
        laddr,
        tx,
        tls: None,
        token: None,
        recv_chunks_timeout: RECV_CHUNKS_TIMEOUT,
        max_decoding_message_size: None,
        max_encoding_message_size: None,
        reuseaddr: true,
        reuseport: false,
        chunk_size: CHUNK_SIZE_LIMIT,
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

    pub fn max_decoding_message_size(mut self, max_decoding_message_size: usize) -> Self {
        self.max_decoding_message_size = Some(max_decoding_message_size);
        self
    }

    pub fn max_encoding_message_size(mut self, max_encoding_message_size: usize) -> Self {
        self.max_encoding_message_size = Some(max_encoding_message_size);
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

    pub fn chunk_size(mut self, chunk_size: usize) -> Self {
        self.chunk_size = chunk_size;
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

        let mut service = DataTransferServer::new(DataTransferService::new(
            self.tx,
            self.recv_chunks_timeout,
            self.chunk_size,
        ));
        if let Some(limit) = self.max_decoding_message_size {
            service = service.max_decoding_message_size(limit);
        }
        if let Some(limit) = self.max_encoding_message_size {
            service = service.max_encoding_message_size(limit);
        }

        let service = InterceptedService::new(service, AuthInterceptor { auth_token });

        log::info!(
            "gRPC DataTransfer is listening on {}://{:?}",
            protocol,
            self.laddr
        );

        #[allow(unused_variables)]
        let server = builder.add_service(service);

        #[cfg(any(feature = "reuseport", feature = "reuseaddr"))]
        #[cfg(feature = "socket2")]
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
    chunk_size: usize,
    recv_chunks_timeout: Duration,
}

impl DataTransferService {
    pub fn new(tx: TX, recv_chunks_timeout: Duration, chunk_size: usize) -> Self {
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
            chunk_size,
            recv_chunks_timeout,
        }
    }

    #[inline]
    fn chunk_empty_result() -> Response<transferpb::Message> {
        let resp = transferpb::Message {
            ..Default::default()
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

            let (priority, data) = if let Some((_, priority, data)) =
                self.chunked_buffer.merge(req, remote_addr, None).await
            {
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
        let id = req.id;
        let (priority, data) = if let Some((_, priority, data)) =
            self.chunked_buffer.merge(req, remote_addr, None).await
        {
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
            id,
            data: Some(res),
            ..Default::default()
        };
        Ok(Response::new(resp))
    }

    type DuplexTransferStream = ResponseStream;

    #[inline]
    async fn duplex_transfer(
        &self,
        request: tonic::Request<tonic::Streaming<transferpb::Message>>,
    ) -> SendResult<Self::DuplexTransferStream> {
        let remote_addr = request.remote_addr();
        let tx = self.tx.clone();
        let mut stream = request.into_inner();
        let (resp_tx, resp_rx) =
            tokio::sync::mpsc::channel::<Result<transferpb::Message, Status>>(100_000);

        let recv_chunks_timeout = self.recv_chunks_timeout;
        let chunk_size = self.chunk_size;
        #[cfg(feature = "rate")]
        let counter = self.counter.clone();
        tokio::spawn(async move {
            let chunked_buffer = ChunkedBuffer::new(recv_chunks_timeout);
            while let Some(req) = stream.next().await {
                log::trace!("Request: {:?}", req);

                let req = match req {
                    Ok(r) => r,
                    Err(e) => {
                        log::debug!("request error, {:?}", e);
                        break;
                    }
                };

                let (id, priority, data) = if let Some((id, priority, data)) =
                    chunked_buffer.merge(req, remote_addr, None).await
                {
                    (id, priority, data)
                } else {
                    continue;
                };

                #[cfg(feature = "rate")]
                counter.inc();

                let mut tx = tx.clone();
                let resp_tx = resp_tx.clone();
                tokio::spawn(async move {
                    let (res_tx, res_rx) = oneshot::channel();
                    if let Err(e) = tx.send((priority, (data, Some(res_tx)))).await {
                        resp_send_error(resp_tx, id, e.to_string()).await;
                    } else {
                        let res = res_rx.await;
                        match res {
                            Err(e) => {
                                resp_send_error(resp_tx, id, e.to_string()).await;
                            }
                            Ok(Err(e)) => {
                                resp_send_error(resp_tx, id, e.to_string()).await;
                            }
                            Ok(Ok(data)) => {
                                for resp in
                                    split_into_chunks(id, data.as_slice(), priority, chunk_size)
                                {
                                    if let Err(e) = resp_tx.send(Ok(resp)).await {
                                        log::warn!("send response result error, {}", e);
                                    }
                                }
                            }
                        }
                    }
                });
            }
        });

        let out_stream = ReceiverStream::new(resp_rx);

        Ok(Response::new(
            Box::pin(out_stream) as Self::DuplexTransferStream
        ))
    }
}

async fn resp_send_error<E: Into<String>>(
    resp_tx: tokio::sync::mpsc::Sender<Result<transferpb::Message, Status>>,
    id: Id,
    err: E,
) {
    let resp = transferpb::Message {
        id,
        err: Some(err.into()),
        ..Default::default()
    };
    if let Err(e) = resp_tx.send(Ok(resp)).await {
        log::warn!("send response result error, {}", e);
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
#[cfg(feature = "socket2")]
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn dummy_tx() -> TX {
        let (tx, _rx) = mpsc::priority_channel::<
            Priority,
            (Vec<u8>, Option<oneshot::Sender<Result<Vec<u8>>>>),
        >(10);
        tx
    }

    // ===== server() function tests =====

    #[test]
    fn test_server_defaults() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx());
        assert_eq!(srv.laddr, addr);
        assert!(srv.tls.is_none());
        assert!(srv.token.is_none());
        assert_eq!(srv.recv_chunks_timeout, RECV_CHUNKS_TIMEOUT);
        assert!(srv.max_decoding_message_size.is_none());
        assert!(srv.max_encoding_message_size.is_none());
        assert!(srv.reuseaddr);
        assert!(!srv.reuseport);
        assert_eq!(srv.chunk_size, CHUNK_SIZE_LIMIT);
    }

    // ===== Server builder method tests =====

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_server_tls() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let tls_config = TLS {
            server_cert: "cert.pem".to_string(),
            server_key: "key.pem".to_string(),
            client_ca: None,
            client_domain: None,
        };
        let srv = server(addr, dummy_tx()).tls(tls_config.clone());
        assert!(srv.tls.is_some());
        let tls = srv.tls.unwrap();
        assert_eq!(tls.server_cert, "cert.pem");
        assert_eq!(tls.server_key, "key.pem");
        assert!(tls.client_ca.is_none());
        assert!(tls.client_domain.is_none());
    }

    #[test]
    fn test_server_token() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).token("secret123".to_string());
        assert_eq!(srv.token, Some("secret123".to_string()));
    }

    #[test]
    fn test_server_recv_chunks_timeout() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).recv_chunks_timeout(Duration::from_secs(15));
        assert_eq!(srv.recv_chunks_timeout, Duration::from_secs(15));
    }

    #[test]
    fn test_server_chunk_size() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).chunk_size(4096);
        assert_eq!(srv.chunk_size, 4096);
    }

    #[test]
    fn test_server_max_decoding_message_size() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).max_decoding_message_size(1024 * 1024 * 2);
        assert_eq!(srv.max_decoding_message_size, Some(1024 * 1024 * 2));
    }

    #[test]
    fn test_server_max_encoding_message_size() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).max_encoding_message_size(1024 * 1024 * 4);
        assert_eq!(srv.max_encoding_message_size, Some(1024 * 1024 * 4));
    }

    #[test]
    fn test_server_reuseaddr() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).reuseaddr(false);
        assert!(!srv.reuseaddr);
    }

    #[test]
    fn test_server_reuseport() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx()).reuseport(true);
        assert!(srv.reuseport);
    }

    #[test]
    fn test_server_builder_chaining() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let srv = server(addr, dummy_tx())
            .chunk_size(2048)
            .recv_chunks_timeout(Duration::from_secs(10))
            .token("token".to_string());
        assert_eq!(srv.chunk_size, 2048);
        assert_eq!(srv.recv_chunks_timeout, Duration::from_secs(10));
        assert_eq!(srv.token, Some("token".to_string()));
    }

    // ===== TLS struct tests =====

    #[test]
    fn test_tls_struct_defaults() {
        let tls = TLS {
            server_cert: "cert.pem".to_string(),
            server_key: "key.pem".to_string(),
            client_ca: None,
            client_domain: None,
        };
        assert_eq!(tls.server_cert, "cert.pem");
        assert_eq!(tls.server_key, "key.pem");
        assert!(tls.client_ca.is_none());
        assert!(tls.client_domain.is_none());
    }

    #[test]
    fn test_tls_struct_with_client_auth() {
        let tls = TLS {
            server_cert: "server.crt".to_string(),
            server_key: "server.key".to_string(),
            client_ca: Some("ca.crt".to_string()),
            client_domain: Some("client.example.com".to_string()),
        };
        assert_eq!(tls.client_ca, Some("ca.crt".to_string()));
        assert_eq!(tls.client_domain, Some("client.example.com".to_string()));
    }

    // ===== DataTransferService tests =====

    #[tokio::test]
    async fn test_data_transfer_service_new() {
        let (tx, _rx) = mpsc::priority_channel::<
            Priority,
            (Vec<u8>, Option<oneshot::Sender<Result<Vec<u8>>>>),
        >(10);
        let service = DataTransferService::new(tx, Duration::from_secs(30), CHUNK_SIZE_LIMIT);
        assert_eq!(service.recv_chunks_timeout, Duration::from_secs(30));
        assert_eq!(service.chunk_size, CHUNK_SIZE_LIMIT);
    }

    #[test]
    fn test_data_transfer_service_chunk_empty_result() {
        let resp = DataTransferService::chunk_empty_result();
        assert_eq!(resp.get_ref().id, 0);
        assert!(resp.get_ref().data.is_none());
    }
}
