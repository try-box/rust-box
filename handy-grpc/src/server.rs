use std::net::SocketAddr;

#[cfg(feature = "rate")]
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::channel::oneshot;
use futures::StreamExt;
use tonic::metadata::{Ascii, MetadataValue};
use tonic::service::Interceptor;
use tonic::transport::{Identity, Server, ServerTlsConfig};
use tonic::{Request, Response, Status};

use anyhow::{Error, Result};

use super::transferpb::data_transfer_server::{DataTransfer, DataTransferServer};
use super::transferpb::{self, Empty};
use super::Priority;

type TX = mpsc::Sender<(Priority, Message), mpsc::SendError<(Priority, Message)>>;

pub type Message = (
    transferpb::Message,
    Option<oneshot::Sender<Result<transferpb::Message>>>,
);

pub struct DataTransferService {
    #[cfg(feature = "rate")]
    counter: std::sync::Arc<AtomicUsize>,
    tx: TX,
}

impl DataTransferService {
    pub fn new(tx: TX) -> Self {
        #[cfg(feature = "rate")]
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        #[cfg(feature = "rate_print")]
        {
            let c = counter.clone();
            tokio::spawn(async move {
                let mut last = 0;
                loop {
                    let now = std::time::Instant::now();
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    let curr = c.load(Ordering::SeqCst);
                    log::info!(
                        "total: {}, diff: {} rate: {:?}",
                        curr,
                        (curr - last),
                        (curr - last) as f64 / (now.elapsed().as_millis() as f64 / 1000.0)
                    );
                    last = curr;
                }
            });
        }
        Self {
            #[cfg(feature = "rate")]
            counter,
            tx,
        }
    }
}

#[tonic::async_trait]
impl DataTransfer for DataTransferService {
    #[inline]
    async fn transfer(
        &self,
        request: Request<tonic::Streaming<transferpb::Message>>,
    ) -> Result<Response<Empty>, Status> {
        let mut tx = self.tx.clone();
        let mut stream = request.into_inner();
        while let Some(req) = stream.next().await {
            log::trace!("Request: {:?}", req);
            let req = req?;
            #[cfg(feature = "rate")]
            self.counter.fetch_add(1, Ordering::SeqCst);
            tx.send((req.priority as Priority, (req, None)))
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
        let req = request.into_inner();
        log::trace!("Request: {:?}", req);

        #[cfg(feature = "rate")]
        self.counter.fetch_add(1, Ordering::SeqCst);

        let mut tx = self.tx.clone();
        let (res_tx, res_rx) = oneshot::channel();
        tx.send((req.priority as Priority, (req, Some(res_tx))))
            .await
            .map_err(|e| Status::cancelled(e.to_string()))?;

        let res = res_rx
            .await
            .map_err(|e| Status::cancelled(e.to_string()))?
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(res))
    }
}

pub async fn run(
    laddr: SocketAddr,
    tx: TX,
    tls: Option<TLS>,
    token: Option<String>,
    #[cfg(feature = "reuseaddr")] reuseaddr: bool,
    #[cfg(feature = "reuseport")] reuseport: bool,
) -> Result<()> {
    let mut builder = Server::builder();

    //Check for TLS and generate an identity.
    let (tls_identity, protocol) = if let Some(tls) = tls {
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
    let auth_token = if let Some(token) = token {
        if token.is_empty() {
            return Err(Error::msg("auth token is empty"));
        }
        let token = MetadataValue::try_from(&format!("Bearer {}", token)).map_err(Error::new)?;
        Some(token)
    } else {
        None
    };
    let service = DataTransferServer::with_interceptor(
        DataTransferService::new(tx),
        AuthInterceptor { auth_token },
    );

    log::info!(
        "gRPC DataTransfer is listening on {}://{:?}",
        protocol,
        laddr
    );

    let server = builder.add_service(service);

    #[cfg(any(feature = "reuseport", feature = "reuseaddr"))]
    #[cfg(all(feature = "socket2", feature = "tokio-stream"))]
    {
        let listener = socket2_bind(laddr, 1024, reuseaddr, reuseport)?;
        server.serve_with_incoming(listener).await?;
    }
    #[cfg(not(any(feature = "reuseport", feature = "reuseaddr")))]
    server.serve(laddr).await?;

    Ok(())
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
