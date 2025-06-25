use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use prometheus_client::metrics::family::Family;
use prometheus_client::{encoding::text::encode, metrics::counter::Counter, registry::Registry};
use std::{io, net::SocketAddr, sync::Arc};
use tokio::signal::unix::{signal, SignalKind};
use tokio::{net::TcpListener, pin};
// use futures::future::BoxFuture;

use eq_common::ErrorLabels;

/// All Service's Prometheus metrics in a single object
pub struct PromMetrics {
    /// Shared registry for encoding
    pub registry: Arc<Registry>,
    /// Counter for gRPC requests
    pub grpc_req: Counter<u64>,
    /// Counter for attempted jobs
    pub jobs_attempted: Counter<u64>,
    /// Counter for completed jobs
    pub jobs_finished: Counter<u64>,
    /// Counter for jobs failed
    pub jobs_errors: Family<ErrorLabels, Counter>,
}

impl PromMetrics {
    /// Create a new registry and register default metrics
    pub fn new() -> Self {
        let mut registry = <Registry>::with_prefix("eqs");
        let grpc_req = Counter::default();
        registry.register(
            "grpc_req",
            "Total number of gRPC requests served",
            grpc_req.clone(),
        );

        let jobs_attempted = Counter::default();
        registry.register(
            "jobs_attempted",
            "Total number of jobs started, regardless of status",
            jobs_attempted.clone(),
        );

        let jobs_finished = Counter::default();
        registry.register(
            "jobs_finished",
            "Total number of jobs completed successfully, including retries",
            jobs_finished.clone(),
        );

        let jobs_errors = Family::<ErrorLabels, Counter>::default();
        registry.register(
            "jobs_errors",
            "Jobs failed, labeled by variant, including retries",
            jobs_finished.clone(),
        );

        PromMetrics {
            registry: Arc::new(registry),
            grpc_req,
            jobs_attempted,
            jobs_finished,
            jobs_errors,
        }
    }

    /// Start the HTTP endpoint that serves metrics
    pub async fn serve(self: Arc<Self>, addr: SocketAddr) -> io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        let server = http1::Builder::new();

        while let Ok((stream, _)) = listener.accept().await {
            let metrics = Arc::clone(&self);
            let server_cln = server.clone();
            let mut shutdown = signal(SignalKind::terminate())?;

            tokio::spawn(async move {
                let conn = server_cln.serve_connection(
                    TokioIo::new(stream),
                    service_fn(move |_: Request<_>| {
                        let reg = Arc::clone(&metrics.registry);
                        Box::pin(async move {
                            // render the registry into text
                            let mut buf = String::new();
                            match encode(&mut buf, &reg) {
                                Ok(_) => {
                                    let body = full(Bytes::from(buf));
                                    Ok(Response::builder()
                                        .header(
                                            hyper::header::CONTENT_TYPE,
                                            "application/openmetrics-text; version=1.0.0; charset=utf-8",
                                        )
                                        .body(body)
                                        .expect("Response malformed"))
                                }
                                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
                            }
                        })
                    }),
                );

                pin!(conn);
                tokio::select! {
                    _ = conn.as_mut()      => {},               // connection ended
                    _ = shutdown.recv()    => conn.as_mut().graceful_shutdown(),
                }
            });
        }

        Ok(())
    }
}

/// helper to box a full body with `hyper::Error` as the Error type
fn full(body: Bytes) -> BoxBody<Bytes, hyper::Error> {
    Full::new(body)
        // Full::Error = Infallible, so this map_err is never called
        .map_err(|never| match never {})
        .boxed()
}
