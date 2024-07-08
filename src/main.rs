use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::{fs, io, net::SocketAddr};

use axum::Router;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use routes::get_router;
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_rustls::TlsAcceptor;
use tower::Service as TowerService;

mod auth;
mod routes;
mod store;

fn load_certs(path: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    certs(&mut io::BufReader::new(fs::File::open(path)?)).collect()
}

fn load_keys(path: &str) -> io::Result<PrivateKeyDer<'static>> {
    private_key(&mut io::BufReader::new(fs::File::open(path)?))
        .transpose()
        .unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8100));

    let certs = load_certs("cert.pem")?;
    let key = load_keys("key.pem")?;
    println!("{:?}", key);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    config.alpn_protocols = vec!["h2".as_bytes().to_vec(), "http/1.1".as_bytes().to_vec()];

    let acceptor = TlsAcceptor::from(Arc::new(config));

    let router = get_router();

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let stream = acceptor.accept(stream).await.unwrap();

        let io = TokioIo::new(stream);
        if let Err(err) = Builder::new(TokioExecutor::new())
            .serve_connection(io, ProxyService::new(router.clone()))
            .await
        {
            println!("Error serving connection: {:?}", err);
        }
    }
}

type UpstreamMessage = (
    Request<Incoming>,
    oneshot::Sender<Result<Response<Incoming>, hyper::Error>>,
);

struct UpstreamConnection {
    rx: mpsc::UnboundedReceiver<UpstreamMessage>,
}

impl UpstreamConnection {
    async fn start(mut self, addr: &str) {
        let client_stream = TcpStream::connect(addr).await.unwrap();
        let io = TokioIo::new(client_stream);

        let (mut sender, conn) = hyper::client::conn::http2::handshake(TokioExecutor::new(), io)
            .await
            .unwrap();
        tokio::task::spawn(async move {
            println!("Spawned connection await");
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        loop {
            if let Some((mut req, tx)) = self.rx.recv().await {
                let uri_string = format!(
                    "http://{}{}",
                    addr,
                    req.uri()
                        .path_and_query()
                        .map(|x| x.as_str())
                        .unwrap_or("/")
                );
                *req.uri_mut() = uri_string.parse().unwrap();

                println!("Proxying request {:?}", req.uri().path_and_query());

                tx.send(sender.send_request(req).await).unwrap();
            } else {
                println!("Connection closed, exiting loop");
                break;
            }
        }
    }
}

struct ProxyService {
    dispatch: Mutex<Option<mpsc::UnboundedSender<UpstreamMessage>>>,
    router: Router,
}

impl ProxyService {
    fn new(router: Router) -> Self {
        Self {
            dispatch: Mutex::new(None),
            router,
        }
    }

    fn dispatch(
        &self,
        req: Request<Incoming>,
    ) -> oneshot::Receiver<Result<Response<Incoming>, hyper::Error>> {
        let mut dispatch = self.dispatch.lock().unwrap();
        if dispatch.is_none() {
            let (tx, rx) = mpsc::unbounded_channel();
            let upstream = UpstreamConnection { rx };
            tokio::task::spawn(async move { upstream.start("localhost:15002").await });
            *dispatch = Some(tx);
        }
        let (tx, rx) = oneshot::channel();
        dispatch.as_mut().unwrap().send((req, tx)).unwrap();
        rx
    }
}

impl Service<Request<hyper::body::Incoming>> for ProxyService {
    type Response = Response<axum::body::Body>;

    type Error = hyper::Error;

    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        if req
            .uri()
            .path()
            .starts_with("/spark.connect.SparkConnectService")
        {
            let rx = self.dispatch(req);
            Box::pin(async { Ok(rx.await.unwrap()?.map(axum::body::Body::new)) })
        } else {
            let mut router = self.router.clone();
            Box::pin(async move { Ok(router.call(req).await.unwrap()) })
        }
    }
}
