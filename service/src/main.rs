#![doc = include_str!("../../README.md")]

mod internal;
use eq_common::eqs::inclusion_server::InclusionServer;
use internal::grpc::InclusionServiceArc;
use internal::inclusion::*;
use internal::job::*;
use internal::util::*;

use log::{debug, error, info};
use std::sync::Arc;
use tokio::sync::{mpsc, OnceCell};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    std::env::var("NETWORK_PRIVATE_KEY")
        .expect("NETWORK_PRIVATE_KEY for Succinct Prover env var required");
    let da_node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")
        .expect("CELESTIA_NODE_AUTH_TOKEN env var required");
    let da_node_ws =
        std::env::var("CELESTIA_NODE_HTTP").expect("CELESTIA_NODE_HTTP env var required");
    let db_path = std::env::var("EQ_DB_PATH").expect("EQ_DB_PATH env var required");
    let service_socket: std::net::SocketAddr = std::env::var("EQ_SOCKET")
        .expect("EQ_SOCKET env var required")
        .parse()
        .expect("EQ_SOCKET env var required");

    let db = sled::open(db_path.clone())?;
    let queue_db = db.open_tree("queue")?;
    let finished_db = db.open_tree("finished")?;
    let config_db = db.open_tree("config")?;

    info!("Building clients and service setup");
    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Option<Job>>();
    let inclusion_service = Arc::new(InclusionService::new(
        InclusionServiceConfig {
            da_node_token,
            da_node_ws,
        },
        OnceCell::new(),
        OnceCell::new(),
        config_db.clone(),
        queue_db.clone(),
        finished_db.clone(),
        job_sender.clone(),
    ));

    tokio::spawn({
        let service = inclusion_service.clone();
        async move {
            let program_id = get_program_id().await;
            let zk_client = service.clone().get_zk_client_remote().await;
            debug!("ZK client prepared, acquiring setup");
            let _ = service.get_proof_setup(&program_id, zk_client).await;
            info!("ZK client ready!");
        }
        // TODO: crash whole program if this fails
    });

    debug!("Starting service");
    tokio::spawn({
        let service = inclusion_service.clone();
        async move {
            wait_shutdown_signals().await;
            service.shutdown();
        }
    });

    debug!("Starting service");
    tokio::spawn({
        let service = inclusion_service.clone();
        async move { service.job_worker(job_receiver).await }
    });

    tokio::spawn({
        let service = inclusion_service.clone();
        async move {
            let _ = service.clone().get_da_client().await.map_err(|_| {
                error!("PANIC cannot connect to DA client! Exiting!");
                // TODO shutdown
                std::process::exit(1);
            });
            info!("DA client ready!");
        }
        // TODO: crash whole program if this fails
    });

    debug!("Restarting unfinished jobs");
    for (job_key, queue_data) in queue_db.iter().flatten() {
        let job: Job = bincode::deserialize(&job_key).unwrap();
        debug!("Sending {job:?}");
        if let Ok(job_status) = bincode::deserialize::<JobStatus>(&queue_data) {
            match job_status {
                JobStatus::DataAvailabilityPending
                | JobStatus::DataAvailable(_)
                | JobStatus::ZkProofPending(_) => {
                    let _ = job_sender
                        .send(Some(job))
                        .map_err(|e| error!("Failed to send existing job to worker: {}", e));
                }
                _ => {
                    error!("Unexpected job in queue! DB is in invalid state!")
                }
            }
        }
    }

    info!("Starting gRPC Service");
    Server::builder()
        .add_service(InclusionServer::new(InclusionServiceArc(
            inclusion_service.clone(),
        )))
        .serve(service_socket)
        .await?;

    Ok(())
}
