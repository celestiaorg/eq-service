#![doc = include_str!("../../README.md")]

use jsonrpsee::core::ClientError;
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

use eq_common::{
    create_inclusion_proof_input, InclusionServiceError, KeccakInclusionToDataRootProofInput,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sled::{Transactional, Tree as SledTree};

use base64::{self, Engine};

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

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let namespace_serialized =
            base64::engine::general_purpose::STANDARD.encode(&self.namespace);
        let commitment_serialized =
            base64::engine::general_purpose::STANDARD.encode(&self.commitment);
        f.debug_struct("Job")
            .field("height", &self.height)
            .field("namespace", &namespace_serialized)
            .field("commitment", &commitment_serialized)
            .finish()
    }
}

/// Used as a [Job] state machine for the eq-service.
#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    /// DA inclusion proof data is being awaited
    DataAvalibilityPending,
    /// DA inclusion is processed and ready to send to the ZK prover
    DataAvalibile(KeccakInclusionToDataRootProofInput),
    /// A ZK prover job is ready to run
    ZkProofPending(SuccNetJobId),
    /// A ZK proof is ready, and the [Job] is complete
    // For now we'll use the SP1ProofWithPublicValues as the proof
    // Ideally we only want the public values + whatever is needed to verify the proof
    // They don't seem to provide a type for that.
    ZkProofFinished(SP1ProofWithPublicValues),
    /// A wrapper for any [InclusionServiceError], with:
    /// - Option = None               -> No rety is possilbe (Perminent failure)
    /// - Option = Some(\<retry-able status\>) -> Retry is possilbe, with a JobStatus state to retry with
    Failed(InclusionServiceError, Option<Box<JobStatus>>),
}

impl std::fmt::Debug for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::DataAvalibilityPending => write!(f, "DataAvalibilityPending"),
            JobStatus::DataAvalibile(_) => write!(f, "DataAvalibile"),
            JobStatus::ZkProofPending(_) => write!(f, "ZkProofPending"),
            JobStatus::ZkProofFinished(_) => write!(f, "ZkProofFinished"),
            JobStatus::Failed(_, _) => write!(f, "Failed"),
        }
    }
}

pub struct InclusionService {
    da_client: Arc<Client>,
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
        let job = Job {
            height: request.height,
            namespace: request.namespace.clone(),
            commitment: request.commitment.clone(),
        };
        info!("Received grpc request for: {job:?}");

        // TODO FIXME: before we hit any job, we need to check encoding! some use hex and some use base64.
        // MUST reply error to user w/ tip to use correct encoding

        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // First check proof_tree for completed/failed proofs
        debug!("Checking for job status in finalized proof_tree");
        if let Some(proof_data) = self
            .proof_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let job_status: JobStatus =
                bincode::deserialize(&proof_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::ZkProofFinished(proof) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Complete as i32,
                        response_value: Some(ResponseValue::Proof(
                            bincode::serialize(&proof)
                                .map_err(|e| Status::internal(e.to_string()))?,
                        )),
                    }));
                }
                JobStatus::Failed(error, None) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Failed as i32,
                        response_value: Some(ResponseValue::ErrorMessage(format!("{error:?}"))),
                    }));
                }
                JobStatus::Failed(error, retry_status) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::StatusMessage(format!(
                            "Retyring: {retry_status:?} ||| Previous Error: {error:?}"
                        ))),
                    }));
                }
                _ => {
                    let e = "Proof DB is in invalid state";
                    error!("{e}");
                    return Err(Status::internal(e));
                }
            }
        }

        // Then check queue_tree for pending proofs
        debug!("Checking job status for pending queue_tree");
        if let Some(queue_data) = self
            .queue_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let job_status: JobStatus =
                bincode::deserialize(&queue_data).map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::DataAvalibilityPending => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::StatusMessage(
                            "Gathering NMT Proof from Celestia".to_string(),
                        )),
                    }));
                }
                JobStatus::DataAvalibile(_) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::StatusMessage(
                            "Got NMT from Celestia, awating ZK proof".to_string(),
                        )),
                    }));
                }
                JobStatus::ZkProofPending(job_id) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::InProgress as i32,
                        response_value: Some(ResponseValue::ProofId(job_id.to_vec())),
                    }));
                }
                _ => {
                    let e = "Queue is in invalid state";
                    error!("{e}");
                    return Err(Status::internal(e));
                }
            }
        }

        debug!("Sending job to worker and adding to queue...");
        self.queue_db
            .insert(
                &job_key,
                bincode::serialize(&JobStatus::DataAvalibilityPending)
                    .map_err(|e| Status::internal(e.to_string()))?,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        self.job_sender
            .send(job.clone())
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
    /// A worker that recives [Job]s by a channel and drives them to completion
    /// Each state change is handled for [JobStatus] that creates an atomic unit of
    /// work to be completed async. Once completed, work is commetted into
    /// a queue data base that can be recovered to take up where a job was left off.
    ///
    /// Once the job comes to an ending successful or failed state,
    /// the job is atomically removed from the queue and added to a results data base.
    async fn job_worker(
        &self,
        mut job_receiver: mpsc::UnboundedReceiver<Job>,
    ) -> Result<(), InclusionServiceError> {
        debug!("Job worker started");
        while let Some(job) = job_receiver.recv().await {
            debug!("Job worker received {job:?}",);

            let job_key = bincode::serialize(&job).unwrap();

            if let Some(queue_data) = self.queue_db.get(&job_key).unwrap() {
                let mut job_status: JobStatus = bincode::deserialize(&queue_data).unwrap();
                debug!("Job worker processing with starting status: {job_status:?}");
                match job_status {
                    JobStatus::DataAvalibilityPending => {
                        match get_zk_proof_input_from_da(&job, self.da_client.clone()).await {
                            Ok(proof_input) => {
                                job_status = JobStatus::DataAvalibile(proof_input);
                                self.send_job_with_new_status(
                                    &self.queue_db,
                                    job_key,
                                    job_status,
                                    job,
                                )?;
                            }
                            Err(e) => {
                                error!("{job:?} failed progressing DataAvalibilityPending: {e}");
                                job_status = JobStatus::Failed(e, None);
                                self.finalize_job(job_key, job_status)?;
                            }
                        };
                    }
                    JobStatus::DataAvalibile(proof_input) => {
                        match request_zk_proof(proof_input).await {
                            Ok(zk_job_id) => {
                                job_status = JobStatus::ZkProofPending(zk_job_id);
                                self.send_job_with_new_status(
                                    &self.queue_db,
                                    job_key,
                                    job_status,
                                    job,
                                )?;
                            }
                            Err(e) => {
                                error!("{job:?} failed progressing DataAvalibile: {e}");
                                job_status = JobStatus::Failed(e, None);
                                self.finalize_job(job_key, job_status)?;
                            }
                        };
                    }
                    JobStatus::ZkProofPending(zk_job_id) => {
                        match wait_for_zk_proof(zk_job_id).await {
                            Ok(zk_proof) => {
                                job_status = JobStatus::ZkProofFinished(zk_proof);
                                self.send_job_with_new_status(
                                    &self.queue_db,
                                    job_key,
                                    job_status,
                                    job,
                                )?;
                            }
                            Err(e) => {
                                error!("{job:?} failed progressing ZkProofPending: {e}");
                                job_status = JobStatus::Failed(e, None);
                                self.finalize_job(job_key, job_status)?;
                            }
                        }
                    }
                    JobStatus::ZkProofFinished(_) => (),
                    JobStatus::Failed(_, _) => {
                        // TODO: "Need to impl some way to retry some failures, and report perminent failures here"
                        ()
                    }
                }
            }
        }
        unreachable!("Workers must have exhaustive status matching to handle jobs!")
    }

    /// Atomically move a job from the database queue tree to the proof tree.
    /// This removes the job from any further processing by workers.
    /// The [JobStatus] should be success or failure only
    /// (but this is not enforced or checked at this time)
    fn finalize_job(
        &self,
        job_key: Vec<u8>,
        job_status: JobStatus,
    ) -> Result<(), InclusionServiceError> {
        // TODO: do we want to do a status check here? To prevent accidenily getting into a DB invalid state
        (&self.queue_db, &self.proof_db)
            .transaction(|(queue_tx, proof_tx)| {
                queue_tx.remove(job_key.clone())?.unwrap();
                proof_tx.insert(job_key.clone(), bincode::serialize(&job_status).unwrap())?;
                Ok::<(), sled::transaction::ConflictableTransactionError<InclusionServiceError>>(())
            })
            .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;
        Ok(())
    }

    /// Insert a [JobStatus] into a [SledTree] database
    /// AND `send()` this job back to the `self.job_sender` to schedule more progress.
    /// You likely want to pass `self.some_sled_tree` into `data_base` as input.
    fn send_job_with_new_status(
        &self,
        data_base: &SledTree,
        job_key: Vec<u8>,
        update_status: JobStatus,
        job: Job,
    ) -> Result<(), InclusionServiceError> {
        debug!("Sending {job:?} back with updated status: {update_status:?}");
        data_base
            .insert(
                job_key,
                bincode::serialize(&update_status)
                    .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?,
            )
            .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;
        Ok(self
            .job_sender
            .send(job)
            .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?)
    }
}

/// Connect to the Cestia [Client] and attempt to get a NMP for a [Job].
/// A successful Result indicates that the queue DB contains valid ZKP input
async fn get_zk_proof_input_from_da(
    job: &Job,
    client: Arc<Client>,
) -> Result<KeccakInclusionToDataRootProofInput, InclusionServiceError> {
    debug!("Preparing request to Celestia...");
    let height = job.height;

    let commitment =
        Commitment::new(job.commitment.clone().try_into().map_err(|_| {
            InclusionServiceError::InvalidParameter("Invalid commitment".to_string())
        })?);

    let namespace = Namespace::new_v0(&job.namespace).map_err(|e| {
        InclusionServiceError::InvalidParameter(format!("Invalid namespace: {}", e))
    })?;

    debug!("Getting blob from Celestia...");
    let try_blob = client.blob_get(height, namespace, commitment).await;
    let blob = try_blob
        .inspect_err(|e: &ClientError| {
            match e {
                ClientError::Call(error_object) => {
                    // TODO: make this handle errors much better!  See ErrorCode::ServerError(1){
                    if error_object.message() == "header: not found" {
                        todo!();
                    };
                }
                // TODO: handle other Celestia JSON RPC errors
                _ => (),
            }
        })
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

    debug!("Creating ZK Proof input from Celestia Data...");
    if let Ok(proof_input) = create_inclusion_proof_input(&blob, &header, nmt_multiproofs) {
        return Ok(proof_input);
    }

    error!("Failed to get proof from Celestia - This should be unrechable!");
    Err(InclusionServiceError::CelestiaError(format!(
        "Could not obtain NMT proof of data inclusion"
    )))
}

async fn request_zk_proof(
    proof_input: KeccakInclusionToDataRootProofInput,
) -> Result<SuccNetJobId, InclusionServiceError> {
    let network_prover = ProverClient::builder().network().build();
    let (pk, _vk) = network_prover.setup(KECCAK_INCLUSION_ELF);

    debug!("Preparing prover network request and starting proving...");
    let mut stdin = SP1Stdin::new();
    stdin.write(&proof_input);
    let request_id: SuccNetJobId = network_prover
        .prove(&pk, &stdin)
        .groth16()
        .request_async()
        .await
        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?
        .into();

    Ok(request_id)
}

async fn wait_for_zk_proof(
    request_id: SuccNetJobId,
) -> Result<SP1ProofWithPublicValues, InclusionServiceError> {
    // TODO: can the service hold a single instance of a prover client?
    let network_prover = ProverClient::builder().network().build();
    let _ = network_prover.setup(KECCAK_INCLUSION_ELF);

    debug!("Waiting for proof from prover network...");
    network_prover
        .wait_proof(request_id.into(), None)
        .await
        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    std::env::var("NETWORK_PRIVATE_KEY")
        .expect("NETWORK_PRIVATE_KEY for Succinct Prover env var reqired");
    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")
        .expect("CELESTIA_NODE_AUTH_TOKEN env var required");
    let node_ws = std::env::var("CELESTIA_NODE_WS").expect("CELESTIA_NODE_WS env var required");
    let db_path = std::env::var("EQ_DB_PATH").expect("EQ_DB_PATH env var required");
    let service_socket = std::env::var("EQ_SOCKET").expect("EQ_SOCKET env var required");

    let db = sled::open(db_path)?;
    let queue_tree = db.open_tree("queue")?;
    let proof_tree = db.open_tree("proof")?;

    let client = Client::new(node_ws.as_str(), Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Job>();
    let inclusion_service = InclusionService {
        da_client: Arc::new(client),
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
                match job_status {
                    JobStatus::DataAvalibilityPending => {
                        if let Err(e) = job_sender.send(job) {
                            error!("Failed to send existing job to worker: {}", e);
                        } else {
                            jobs_sent_on_startup += 1;
                        }
                    }
                    JobStatus::ZkProofPending(_) => {
                        if let Err(e) = job_sender.send(job) {
                            error!("Failed to send existing job to worker: {}", e);
                        } else {
                            jobs_sent_on_startup += 1;
                        }
                    }
                    _ => {}
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
