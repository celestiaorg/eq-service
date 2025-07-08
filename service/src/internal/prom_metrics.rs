use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use jsonrpsee::tracing::info;
use prometheus_client::metrics::family::Family;
use prometheus_client::{
    encoding::text::encode, metrics::counter::Counter, metrics::histogram::Histogram,
    registry::Registry,
};
use std::time::Duration;
use std::{io, net::SocketAddr, sync::Arc};
use tokio::sync::Notify;
use tokio::{net::TcpListener, pin};

use eq_common::ErrorLabels;

/// All Service's Prometheus metrics in a single object
pub struct PromMetrics {
    /// Shared registry for encoding
    pub registry: Arc<Registry>,
    /// Shutdown gracefully
    shutdown: Arc<Notify>,
    /// Counter for gRPC requests
    pub grpc_req: Counter<u64>,
    /// Counter for attempted jobs
    pub jobs_attempted: Counter<u64>,
    /// Counter for completed jobs
    pub jobs_finished: Counter<u64>,
    /// Counter for jobs failed
    pub jobs_errors: Family<ErrorLabels, Counter>,
    /// Histogram for ZK proof wait times
    pub zk_proof_wait_time: Histogram,
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
            jobs_errors.clone(),
        );

        let zk_proof_gen_timeout_float = Duration::from_secs(
            std::env::var("PROOF_GEN_TIMEOUT_SECONDS")
                .expect("PROOF_GEN_TIMEOUT_SECONDS env var required")
                .parse()
                .expect("PROOF_GEN_TIMEOUT_SECONDS must be integer"),
        )
        .as_secs_f64();

        let zk_proof_wait_time = Histogram::new(
            // 5% of timeout seconds buckets from 0 to 110% (assuming we may sometimes blow past timeout)
            (1..=22).map(|i| ((i as f64 * 0.05) * zk_proof_gen_timeout_float).floor()),
        );
        registry.register(
            "zk_proof_wait_time",
            "Time taken to wait for ZK proof completion in seconds (Buckets of 5% PROOF_GEN_TIMEOUT_SECONDS env var)",
            zk_proof_wait_time.clone(),
        );

        PromMetrics {
            registry: Arc::new(registry),
            shutdown: Arc::new(Notify::new()),
            grpc_req,
            jobs_attempted,
            jobs_finished,
            jobs_errors,
            zk_proof_wait_time,
        }
    }

    /// Start the HTTP endpoint that serves metrics
    pub async fn serve(self: Arc<Self>, addr: SocketAddr) -> io::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        let server = http1::Builder::new();

        info!("Prometheus serving on {:?}", addr);
        while let Ok((stream, _)) = listener.accept().await {
            let metrics = Arc::clone(&self);
            let server_cln = server.clone();
            let notify = self.shutdown.clone();
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
                                    _ = notify.notified() => {
                    conn.as_mut().graceful_shutdown();
                },
                            }
            });
        }

        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}

/// helper to box a full body with `hyper::Error` as the Error type
fn full(body: Bytes) -> BoxBody<Bytes, hyper::Error> {
    Full::new(body)
        // Full::Error = Infallible, so this map_err is never called
        .map_err(|never| match never {})
        .boxed()
}
