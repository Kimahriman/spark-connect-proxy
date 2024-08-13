use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::{fs, io};

use axum::Router;
use clap::{command, Parser};
use config::ProxyConfig;
use http::header::AUTHORIZATION;
use http::{response, status, StatusCode};
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, Response};

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use log::info;
use routes::get_router;
use rustls_pemfile::{certs, private_key};
use store::{InMemorySessionStore, SessionStore};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_rustls::TlsAcceptor;
use tower::Service as TowerService;

mod auth;
mod config;
mod launcher;
mod routes;
mod store;

/// Start the Spark Connect Proxy server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the config file
    #[arg(short, long)]
    config_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();

    let config = args
        .config_file
        .map(ProxyConfig::from_file)
        .unwrap_or_default();

    let bind_host = config.bind_host.clone().unwrap_or("0.0.0.0".to_string());

    let bind_port = config.get_bind_port();

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", bind_host, bind_port)).await?;
    println!("Listening on http://{:?}", listener.local_addr().unwrap());

    let session_store = Arc::new(InMemorySessionStore::default());
    let router = get_router(&config, session_store.clone());
    let tls_acceptor = load_tls_acceptor(&config)?;

    loop {
        let (stream, _) = listener.accept().await.unwrap();

        info!("Serving new connection");
        let router = router.clone();
        let session_store = session_store.clone();

        if let Some(acceptor) = tls_acceptor.as_ref() {
            let io = TokioIo::new(acceptor.accept(stream).await?);

            tokio::task::spawn(async move {
                // Serve via TLS
                let result = Builder::new(TokioExecutor::new())
                    .serve_connection(io, ProxyService::new(router, session_store))
                    .await;

                if let Err(err) = result {
                    println!("Error serving connection: {:?}", err);
                }
            });
        } else {
            tokio::task::spawn(async move {
                // Serve unencrypted
                let result = Builder::new(TokioExecutor::new())
                    .serve_connection(
                        TokioIo::new(stream),
                        ProxyService::new(router, session_store),
                    )
                    .await;

                if let Err(err) = result {
                    println!("Error serving connection: {:?}", err);
                }
            });
        };
    }
}

fn load_tls_acceptor(config: &ProxyConfig) -> Result<Option<TlsAcceptor>, io::Error> {
    if let Some(tls_config) = &config.tls {
        let certs = certs(&mut io::BufReader::new(fs::File::open(&tls_config.cert)?))
            .collect::<io::Result<Vec<_>>>()?;
        let key = private_key(&mut io::BufReader::new(fs::File::open(&tls_config.key)?))?.unwrap();

        let mut config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

        config.alpn_protocols = vec!["h2".as_bytes().to_vec(), "http/1.1".as_bytes().to_vec()];

        let acceptor = TlsAcceptor::from(Arc::new(config));
        Ok(Some(acceptor))
    } else {
        Ok(None)
    }
}

type UpstreamMessage = (
    Request<Incoming>,
    oneshot::Sender<Result<Response<axum::body::Body>, hyper::Error>>,
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

                tx.send(
                    sender
                        .send_request(req)
                        .await
                        .map(|response| response.map(axum::body::Body::new)),
                )
                .unwrap();
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
    session_store: Arc<dyn SessionStore>,
}

impl ProxyService {
    fn new(router: Router, session_store: Arc<dyn SessionStore>) -> Self {
        Self {
            dispatch: Mutex::new(None),
            router,
            session_store,
        }
    }

    fn dispatch(
        &self,
        req: Request<Incoming>,
    ) -> oneshot::Receiver<Result<Response<axum::body::Body>, hyper::Error>> {
        let mut dispatch = self.dispatch.lock().unwrap();
        let (tx, rx) = oneshot::channel();
        if dispatch.is_none() {
            let authorization = if let Some(auth) = req.headers().get(AUTHORIZATION) {
                auth.to_str().unwrap().to_string()
            } else {
                let response = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(().into())
                    .unwrap();
                tx.send(Ok(response)).unwrap();
                return rx;
            };

            let split = authorization.split_once(' ');
            let token = match split {
                Some(("Bearer", token)) => token,
                _ => {
                    let response = Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(().into())
                        .unwrap();
                    tx.send(Ok(response)).unwrap();
                    return rx;
                }
            };

            if let Some(session) = self.session_store.get_session_by_token(token) {
                let (upstream_sender, upstream_receiver) = mpsc::unbounded_channel();
                let upstream = UpstreamConnection {
                    rx: upstream_receiver,
                };
                tokio::task::spawn(
                    async move { upstream.start(session.addr.unwrap().as_ref()).await },
                );
                *dispatch = Some(upstream_sender);
            } else {
                let response = Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(().into())
                    .unwrap();
                tx.send(Ok(response)).unwrap();
                return rx;
            }
        }
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
