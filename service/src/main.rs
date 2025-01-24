use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

pub mod eqs {
    include!("generated/eqs.rs");
}
use eqs::inclusion_server::{Inclusion, InclusionServer};
use eqs::{
    get_keccak_inclusion_response::{ResponseValue, Status as ResponseStatus},
    GetKeccakInclusionRequest, GetKeccakInclusionResponse,
};

use celestia_rpc::{BlobClient, Client, HeaderClient};
use celestia_types::blob::Commitment;
use celestia_types::nmt::Namespace;
use sp1_sdk::{Prover, ProverClient, SP1ProofWithPublicValues, SP1Stdin};
use tokio::sync::mpsc;

use eq_common::{create_inclusion_proof_input, InclusionServiceError};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sled::Tree as SledTree;

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);
type SuccNetJobId = [u8; 32];

#[derive(Serialize, Deserialize, Clone)]
pub struct Job {
    pub height: u64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    // Before it goes to Prover Network, it might be hanging on Celestia
    Waiting,
    // The Succinct Network job ID
    Pending(SuccNetJobId),
    // For now we'll use the SP1ProofWithPublicValues as the proof
    // Ideally we only want the public values + whatever is needed to verify the proof
    // They don't seem to provide a type for that.
    Completed(SP1ProofWithPublicValues),
    Failed(String),
}
pub struct InclusionService {
    client: Arc<Client>,
    job_sender: mpsc::UnboundedSender<Job>,
    queue_db: SledTree,
    proof_db: SledTree,
}

#[tonic::async_trait]
impl Inclusion for InclusionService {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        let request = request.into_inner();
        info!(
            "Received grpc request for commitment: {}",
            hex::encode(request.commitment.clone())
        );
        let job = Job {
            height: request.height,
            namespace: request.namespace.clone(),
            commitment: request.commitment.clone(),
        };
        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // First check proof_tree for completed/failed proofs
        debug!("Checking proof_tree for finished/failed proofs");
        if let Some(proof_data) = self
            .proof_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let job_status: JobStatus =
                bincode::deserialize(&proof_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::Completed(proof) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Complete as i32,
                        response_value: Some(ResponseValue::Proof(
                            bincode::serialize(&proof)
                                .map_err(|e| Status::internal(e.to_string()))?,
                        )),
                    }));
                }
                JobStatus::Failed(error) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Failed as i32,
                        response_value: Some(ResponseValue::ErrorMessage(error)),
                    }));
                }
                _ => return Err(Status::internal("Invalid state in proof_tree")),
            }
        }

        // Then check queue_tree for pending proofs
        debug!("Checking queue_tree for pending proofs");
        if let Some(queue_data) = self
            .queue_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let job_status: JobStatus =
                bincode::deserialize(&queue_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::Pending(job_id) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::ProofId(job_id.to_vec())),
                    }));
                }
                JobStatus::Waiting => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::StatusMessage("queued".to_string())),
                    }));
                }
                _ => {
                    error!("Expected job to be pending or waiting");
                    return Err(Status::internal("Expected job to be pending or waiting"));
                }
            }
        }

        debug!("Sending job to worker and adding to queue...");
        self.job_sender
            .send(job.clone())
            .map_err(|e| Status::internal(e.to_string()))?;

        let waiting_status = JobStatus::Waiting;
        self.queue_db
            .insert(
                &job_key,
                bincode::serialize(&waiting_status).map_err(|e| Status::internal(e.to_string()))?,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        debug!("Returning waiting response...");
        Ok(Response::new(GetKeccakInclusionResponse {
            status: ResponseStatus::Waiting as i32,
            response_value: Some(ResponseValue::StatusMessage(
                "sent to proof worker".to_string(),
            )),
        }))
    }
}

#[tonic::async_trait]
impl Inclusion for Arc<InclusionService> {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        (**self).get_keccak_inclusion(request).await
    }
}

impl InclusionService {
    async fn job_worker(&self, mut job_receiver: mpsc::UnboundedReceiver<Job>) {
        info!("Job worker started");
        while let Some(job) = job_receiver.recv().await {
            debug!(
                "job worker received job for commitment: {}",
                hex::encode(job.commitment.clone())
            );
            let client = Arc::clone(&self.client);
            tokio::spawn(prove(
                job,
                client,
                self.queue_db.clone(),
                self.proof_db.clone(),
            ));
        }
    }
}

async fn prove(
    job: Job,
    client: Arc<Client>,
    queue_tree: SledTree,
    proof_tree: SledTree,
) -> Result<(), InclusionServiceError> {
    let network_prover = ProverClient::builder().network().build();
    let (pk, vk) = network_prover.setup(KECCAK_INCLUSION_ELF);

    let from_queue_tree: Option<JobStatus> = match queue_tree
        .get(&bincode::serialize(&job).map_err(|e| {
            InclusionServiceError::GeneralError(format!("Failed to serialize job: {}", e))
        })?)
        .map_err(|e| {
            InclusionServiceError::GeneralError(format!("Failed to get job from queue: {}", e))
        })? {
        Some(job_status_bytes) => bincode::deserialize(&job_status_bytes).map_err(|e| {
            InclusionServiceError::GeneralError(format!("Failed to deserialize job status: {}", e))
        })?,
        None => None,
    };

    let prover_network_job_id: Vec<u8> =
        if let Some(JobStatus::Pending(prover_network_job_id)) = from_queue_tree {
            prover_network_job_id.to_vec()
        } else {
            debug!("Preparing request to Celestia...");
            let height = job.height;

            let commitment = Commitment::new(job.commitment.clone().try_into().map_err(|_| {
                InclusionServiceError::InvalidParameter("Invalid commitment".to_string())
            })?);

            let namespace = Namespace::new_v0(&job.namespace).map_err(|e| {
                InclusionServiceError::InvalidParameter(format!("Invalid namespace: {}", e))
            })?;

            debug!("Getting blob from Celestia...");
            let blob = client
                .blob_get(height, namespace, commitment)
                .await
                .map_err(|e| {
                    error!("Failed to get blob from Celestia: {}", e);
                    InclusionServiceError::CelestiaError(e.to_string())
                })?;

            debug!("Getting header from Celestia...");
            let header = client
                .header_get_by_height(height)
                .await
                .map_err(|e| InclusionServiceError::CelestiaError(e.to_string()))?;

            debug!("Getting NMT multiproofs from Celestia...");
            let nmt_multiproofs = client
                .blob_get_proof(height, namespace, commitment)
                .await
                .map_err(|e| {
                    error!("Failed to get blob proof from Celestia: {}", e);
                    InclusionServiceError::CelestiaError(e.to_string())
                })?;

            debug!("Preparing prover network request and starting proving...");
            let inclusion_proof_input =
                create_inclusion_proof_input(&blob, &header, nmt_multiproofs)
                    .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

            let mut stdin = SP1Stdin::new();
            stdin.write(&inclusion_proof_input);
            let request_id: [u8; 32] = network_prover
                .prove(&pk, &stdin)
                .groth16()
                .request_async()
                .await
                .unwrap() // TODO: Handle this error
                .into();

            debug!("Storing job in queue_tree...");
            // Store in queue_tree
            let serialized_status =
                bincode::serialize(&JobStatus::Pending(request_id)).map_err(|e| {
                    InclusionServiceError::InvalidParameter(format!(
                        "Failed to serialize job status: {}",
                        e
                    ))
                })?;

            queue_tree
                .insert(
                    &bincode::serialize(&job)
                        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?,
                    serialized_status,
                )
                .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

            request_id.to_vec()
        };

    debug!("Waiting for proof from prover network...");
    let prover_network_job_id: [u8; 32] = prover_network_job_id.try_into().map_err(|e| {
        InclusionServiceError::GeneralError(format!(
            "Failed to convert prover network job id to [u8; 32]"
        ))
    })?;
    let proof = network_prover
        .wait_proof(prover_network_job_id.into(), None)
        .await;

    debug!("Storing proof in proof_tree...");
    let job_status = match proof {
        Ok(proof) => JobStatus::Completed(proof),
        Err(e) => JobStatus::Failed(e.to_string()),
    };
    let serialized_status = bincode::serialize(&job_status).map_err(|e| {
        InclusionServiceError::GeneralError(format!("Failed to serialize job status: {}", e))
    })?;
    proof_tree
        .insert(
            &bincode::serialize(&job)
                .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?,
            serialized_status,
        )
        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

    // Remove job from queue_tree after storing in proof_tree
    queue_tree
        .remove(
            &bincode::serialize(&job)
                .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?,
        )
        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("Token not provided");
    let node_ws = std::env::var("CELESTIA_NODE_WS").expect("Token not provided");
    let db_path = std::env::var("EQ_DB_PATH")?;
    let service_socket = std::env::var("EQ_SOCKET")?;

    let db = sled::open(db_path)?;
    let queue_tree = db.open_tree("queue")?;
    let proof_tree = db.open_tree("proof")?;

    let client = Client::new(node_ws.as_str(), Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Job>();
    let inclusion_service = InclusionService {
        client: Arc::new(client),
        queue_db: queue_tree.clone(),
        proof_db: proof_tree.clone(),
        job_sender: job_sender.clone(),
    };

    let inclusion_service = Arc::new(inclusion_service);

    tokio::spawn({
        let service = Arc::clone(&inclusion_service);
        async move { service.job_worker(job_receiver).await }
    });

    let mut jobs_sent_on_startup = 0;
    // Process any existing jobs in the queue
    for entry_result in queue_tree.iter() {
        if let Ok((job_key, queue_data)) = entry_result {
            let job: Job = bincode::deserialize(&job_key).unwrap();
            if let Ok(job_status) = bincode::deserialize::<JobStatus>(&queue_data) {
                if let JobStatus::Pending(job_id) = job_status {
                    if let Err(e) = job_sender.send(job) {
                        error!("Failed to send existing job to worker: {}", e);
                    } else {
                        jobs_sent_on_startup += 1;
                    }
                }
            }
        }
    }

    info!("Sent {} jobs on startup", jobs_sent_on_startup);

    let addr = service_socket.parse()?;

    Server::builder()
        .add_service(InclusionServer::new(Arc::clone(&inclusion_service)))
        .serve(addr)
        .await?;

    Ok(())
}
