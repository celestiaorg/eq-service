#![doc = include_str!("../../README.md")]

use jsonrpsee::core::ClientError as JsonRpcError;
use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

use eq_common::eqs::inclusion_server::{Inclusion, InclusionServer};
use eq_common::eqs::{
    get_keccak_inclusion_response::{ResponseValue, Status as ResponseStatus},
    GetKeccakInclusionRequest, GetKeccakInclusionResponse,
};

use celestia_rpc::{BlobClient, Client as CelestiaJSONClient, HeaderClient};
use celestia_types::{blob::Commitment, block::Height as BlockHeight, nmt::Namespace};
use sp1_sdk::{NetworkProver as SP1NetworkProver, Prover, SP1ProofWithPublicValues, SP1Stdin};
use tokio::sync::mpsc;

use eq_common::{
    create_inclusion_proof_input, InclusionServiceError, KeccakInclusionToDataRootProofInput,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sled::{Transactional, Tree as SledTree};

use base64::Engine;

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);
type SuccNetJobId = [u8; 32];

#[derive(Serialize, Deserialize, Clone)]
struct Job {
    height: BlockHeight,
    namespace: Namespace,
    commitment: Commitment,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let namespace_string;
        if let Some(namespace) = &self.namespace.id_v0() {
            namespace_string = base64::engine::general_purpose::STANDARD.encode(namespace);
        } else {
            namespace_string = "Invalid v0 ID".to_string()
        }
        let commitment_string =
            base64::engine::general_purpose::STANDARD.encode(&self.commitment.hash());
        f.debug_struct("Job")
            .field("height", &self.height.value())
            .field("namespace", &namespace_string)
            .field("commitment", &commitment_string)
            .finish()
    }
}

/// Used as a [Job] state machine for the eq-service.
#[derive(Serialize, Deserialize)]
enum JobStatus {
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
    /// - Option = None               -> No retry is possilbe (Perminent failure)
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

struct InclusionService {
    da_client: Arc<CelestiaJSONClient>,
    sp1_client: Arc<SP1NetworkProver>,
    proof_setup: SP1ProofSetup,
    queue_db: SledTree,
    proof_db: SledTree,
    job_sender: mpsc::UnboundedSender<Job>,
}

#[allow(dead_code)]
struct SP1ProofSetup {
    pk: sp1_sdk::SP1ProvingKey,
    vk: sp1_sdk::SP1VerifyingKey,
}

impl From<(sp1_sdk::SP1ProvingKey, sp1_sdk::SP1VerifyingKey)> for SP1ProofSetup {
    fn from(tuple: (sp1_sdk::SP1ProvingKey, sp1_sdk::SP1VerifyingKey)) -> Self {
        Self {
            pk: tuple.0,
            vk: tuple.1,
        }
    }
}

// I hate this workaround. Kill it with fire.
struct InclusionServiceArc(Arc<InclusionService>);

#[tonic::async_trait]
impl Inclusion for InclusionServiceArc {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        let request = request.into_inner();
        let job = Job {
            height: request
                .height
                .try_into()
                .map_err(|_| Status::invalid_argument("Block Height must be u64"))?,

            // TODO: should we have some handling of versions here?
            namespace: Namespace::new_v0(&request.namespace).map_err(|_| {
                Status::invalid_argument("Namespace v0 expected! Must be 32 bytes, check encoding")
            })?,
            commitment: Commitment::new(request.commitment.try_into().map_err(|_| {
                Status::invalid_argument("Commitment must be 32 bytes, check encoding")
            })?),
        };

        info!("Received grpc request for: {job:?}");

        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // First check proof_tree for completed/failed proofs
        if let Some(proof_data) = self
            .0
            .proof_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            debug!("Job finalized proof");
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
                    // TODO: retry or not?
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::StatusMessage(format!(
                            "Retryring: {retry_status:?} ||| Previous Error: {error:?}"
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
        if let Some(queue_data) = self
            .0
            .queue_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            debug!("Job in pending queue");
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

        info!("New {job:?} sending to worker and adding to queue");
        self.0
            .queue_db
            .insert(
                &job_key,
                bincode::serialize(&JobStatus::DataAvalibilityPending)
                    .map_err(|e| Status::internal(e.to_string()))?,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        self.0
            .job_sender
            .send(job.clone())
            .map_err(|e| Status::internal(e.to_string()))?;

        debug!("Returning waiting response");
        Ok(Response::new(GetKeccakInclusionResponse {
            status: ResponseStatus::Waiting as i32,
            response_value: Some(ResponseValue::StatusMessage(
                "sent to proof worker".to_string(),
            )),
        }))
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
    async fn job_worker(self: Arc<Self>, mut job_receiver: mpsc::UnboundedReceiver<Job>) {
        debug!("Job worker started");
        while let Some(job) = job_receiver.recv().await {
            let service = self.clone();
            tokio::spawn(async move {
                debug!("Job worker received {job:?}",);
                let _ = service.prove(job).await; //Don't return with "?", we run keep looping
            });
        }
        unreachable!("Worker loops on tasks")
    }

    /// The main service task: produce a proof based on a [Job] requested.
    async fn prove(&self, job: Job) -> Result<(), InclusionServiceError> {
        let job_key = bincode::serialize(&job).unwrap();
        Ok(
            if let Some(queue_data) = self.queue_db.get(&job_key).unwrap() {
                let mut job_status: JobStatus = bincode::deserialize(&queue_data).unwrap();
                debug!("Job worker processing with starting status: {job_status:?}");
                match job_status {
                    JobStatus::DataAvalibilityPending => {
                        self.get_zk_proof_input_from_da(&job, job_key, self.da_client.clone())
                            .await?
                    }
                    JobStatus::DataAvalibile(proof_input) => {
                        match self.request_zk_proof(proof_input).await {
                            Ok(zk_job_id) => {
                                debug!("DA data -> zk input ready");
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
                                self.finalize_job(&job_key, job_status)?;
                            }
                        };
                    }
                    JobStatus::ZkProofPending(zk_job_id) => {
                        match self.wait_for_zk_proof(zk_job_id).await {
                            Ok(zk_proof) => {
                                info!("🎉 {job:?} Finished!");
                                job_status = JobStatus::ZkProofFinished(zk_proof);
                                self.finalize_job(&job_key, job_status)?;
                            }
                            Err(e) => {
                                error!("{job:?} failed progressing ZkProofPending: {e}");
                                job_status = JobStatus::Failed(e, None);
                                self.finalize_job(&job_key, job_status)?;
                            }
                        }
                    }
                    JobStatus::ZkProofFinished(_) => (),
                    JobStatus::Failed(_, _) => {
                        // TODO: "Need to impl some way to retry some failures, and report perminent failures here"
                        ()
                    }
                }
            },
        )
    }

    /// Connect to the Cestia [CelestiaJSONClient] and attempt to get a NMP for a [Job].
    /// A successful Result indicates that the queue DB contains valid ZKP input
    async fn get_zk_proof_input_from_da(
        &self,
        job: &Job,
        job_key: Vec<u8>,
        client: Arc<CelestiaJSONClient>,
    ) -> Result<(), InclusionServiceError> {
        debug!("Preparing request to Celestia");
        let blob = client
            .blob_get(job.height.into(), job.namespace, job.commitment)
            .await
            .map_err(|e| self.handle_da_client_error(e, &job, &job_key))?;

        let header = client
            .header_get_by_height(job.height.into())
            .await
            .map_err(|e| {
                error!("Failed to get header proof from Celestia: {}", e);
                InclusionServiceError::CelestiaError(e.to_string())
            })?;

        let nmt_multiproofs = client
            .blob_get_proof(job.height.into(), job.namespace, job.commitment)
            .await
            .map_err(|e| {
                error!("Failed to get blob proof from Celestia: {}", e);
                InclusionServiceError::CelestiaError(e.to_string())
            })?;

        debug!("Creating ZK Proof input from Celestia Data");
        if let Ok(proof_input) = create_inclusion_proof_input(&blob, &header, nmt_multiproofs) {
            self.send_job_with_new_status(
                &self.queue_db,
                job_key,
                JobStatus::DataAvalibile(proof_input),
                job.clone(),
            )?;
            return Ok(());
        }

        error!("Failed to get proof from Celestia - This should be unrechable!");
        Err(InclusionServiceError::CelestiaError(format!(
            "Could not obtain NMT proof of data inclusion"
        )))
    }

    /// Helper function to handle error from a [jsonrpsee] based DA client.
    /// If we can handle the client error, we wrap Ok() it
    /// If there is another issues (like writting to the DB) we Err
    fn handle_da_client_error(
        &self,
        da_client_error: JsonRpcError,
        job: &Job,
        job_key: &Vec<u8>,
    ) -> InclusionServiceError {
        error!("Celestia Client error: {da_client_error}");
        let (e, job_status);
        match da_client_error {
            JsonRpcError::Call(error_object) => {
                // TODO: make this handle errors much better! JSON stringyness is a problem!
                if error_object.message() == "header: not found" {
                    e = InclusionServiceError::CelestiaError("header: not found. Likely Celestia Node is missing blob, although it exists on the network overall.".to_string());
                    error!("{job:?} failed, recoverable: {e}");
                    job_status = JobStatus::Failed(
                        e.clone(),
                        Some(JobStatus::DataAvalibilityPending.into()),
                    );
                } else if error_object.message() == "blob: not found" {
                    e = InclusionServiceError::CelestiaError("blob: not found".to_string());
                    error!("{job:?} failed, not recoverable: {e}");
                    job_status = JobStatus::Failed(e.clone(), None);
                } else {
                    e = InclusionServiceError::CelestiaError("UNKNOWN client error".to_string());
                    error!("{job:?} failed, not recoverable: {e}");
                    job_status = JobStatus::Failed(e.clone(), None);
                }
            }
            // TODO: handle other Celestia JSON RPC errors
            _ => {
                e = InclusionServiceError::CelestiaError(
                    "Unhandled Celestia SDK error".to_string(),
                );
                error!("{job:?} failed, not recoverable: {e}");
                job_status = JobStatus::Failed(e.clone(), None);
            }
        };
        match self.finalize_job(job_key, job_status) {
            Ok(_) => return e,
            Err(internal_err) => return internal_err,
        };
    }

    /// Start a proof request from Succinct's prover network
    async fn request_zk_proof(
        &self,
        proof_input: KeccakInclusionToDataRootProofInput,
    ) -> Result<SuccNetJobId, InclusionServiceError> {
        debug!("Preparing prover network request and starting proving");

        let mut stdin = SP1Stdin::new();
        stdin.write(&proof_input);
        let request_id: SuccNetJobId = self
            .sp1_client
            .prove(&self.proof_setup.pk, &stdin)
            .groth16()
            .request_async()
            .await
            .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?
            .into();

        Ok(request_id)
    }

    /// Await a proof request from Succinct's prover network
    async fn wait_for_zk_proof(
        &self,
        request_id: SuccNetJobId,
    ) -> Result<SP1ProofWithPublicValues, InclusionServiceError> {
        debug!("Waiting for proof from prover network");

        self.sp1_client
            .wait_proof(request_id.into(), None)
            .await
            .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))
    }

    /// Atomically move a job from the database queue tree to the proof tree.
    /// This removes the job from any further processing by workers.
    /// The [JobStatus] should be success or failure only
    /// (but this is not enforced or checked at this time)
    fn finalize_job(
        &self,
        job_key: &Vec<u8>,
        job_status: JobStatus,
    ) -> Result<(), InclusionServiceError> {
        // TODO: do we want to do a status check here? To prevent accidenily getting into a DB invalid state
        (&self.queue_db, &self.proof_db)
            .transaction(|(queue_tx, proof_tx)| {
                queue_tx.remove(job_key.clone())?;
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    std::env::var("NETWORK_PRIVATE_KEY")
        .expect("NETWORK_PRIVATE_KEY for Succinct Prover env var reqired");
    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")
        .expect("CELESTIA_NODE_AUTH_TOKEN env var required");
    let node_ws = std::env::var("CELESTIA_NODE_WS").expect("CELESTIA_NODE_WS env var required");
    let db_path = std::env::var("EQ_DB_PATH").expect("EQ_DB_PATH env var required");
    let service_socket: std::net::SocketAddr = std::env::var("EQ_SOCKET")
        .expect("EQ_SOCKET env var required")
        .parse()
        .expect("EQ_SOCKET env var reqired");

    let db = sled::open(db_path)?;
    let queue_tree = db.open_tree("queue")?;
    let proof_tree = db.open_tree("proof")?;

    info!("Running preflight setup");

    debug!("Building DA client");
    let da_client = Arc::new(
        CelestiaJSONClient::new(node_ws.as_str(), Some(&node_token))
            .await
            .expect("Failed creating celestia rpc client"),
    );

    debug!("Building prover client");
    let sp1_client = Arc::new(
        tokio::task::spawn_blocking(|| sp1_sdk::ProverClient::builder().network().build()).await?,
    );

    debug!("Generating prover setup");
    let prover_clone = sp1_client.clone();
    let proof_setup: SP1ProofSetup =
        tokio::task::spawn_blocking(move || prover_clone.setup(KECCAK_INCLUSION_ELF).into())
            .await?;

    debug!("Starting service");
    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Job>();
    let inclusion_service = Arc::new(InclusionService {
        da_client,
        sp1_client,
        proof_setup,
        queue_db: queue_tree.clone(),
        proof_db: proof_tree.clone(),
        job_sender: job_sender.clone(),
    });

    tokio::spawn({
        let service = inclusion_service.clone();
        async move { service.job_worker(job_receiver).await }
    });

    debug!("Restarting unfinised jobs");
    for entry_result in queue_tree.iter() {
        if let Ok((job_key, queue_data)) = entry_result {
            let job: Job = bincode::deserialize(&job_key).unwrap();
            debug!("Sending {job:?}");
            if let Ok(job_status) = bincode::deserialize::<JobStatus>(&queue_data) {
                match job_status {
                    JobStatus::DataAvalibilityPending
                    | JobStatus::DataAvalibile(_)
                    | JobStatus::ZkProofPending(_) => {
                        let _ = job_sender
                            .send(job)
                            .map_err(|e| error!("Failed to send existing job to worker: {}", e));
                    }
                    _ => {
                        error!("Unexpected job in queue! DB is in invalid state!")
                    }
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
