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
use tokio::sync::{mpsc, OnceCell};

use eq_common::{
    create_inclusion_proof_input, InclusionServiceError, KeccakInclusionToDataRootProofInput,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sled::{Transactional, Tree as SledTree};

use base64::Engine;
use hex;
use sha3::{Digest, Sha3_256};

/// A Succunct Prover Network request ID.
/// See: https://docs.succinct.xyz/docs/generating-proofs/prover-network/usage
type SuccNetJobId = [u8; 32];

/// A SHA3 256 bit hash of a zkVM program's ELF.
type SuccNetProgramId = [u8; 32];

/// Hardcoded ELF binary for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);
/// Hardcoded ID for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_ID: OnceCell<SuccNetProgramId> = OnceCell::const_new();

/// Hardcoded setup for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_SETUP: OnceCell<Arc<SP1ProofSetup>> = OnceCell::const_new();

#[allow(dead_code)]
#[derive(Serialize, Deserialize)]
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

/// The main service, depends on external DA and ZK clients internally!
struct InclusionService {
    config: InclusionServiceConfig,
    da_client_handle: OnceCell<Arc<CelestiaJSONClient>>,
    zk_client_handle: OnceCell<Arc<SP1NetworkProver>>,
    config_db: SledTree,
    queue_db: SledTree,
    finished_db: SledTree,
    job_sender: mpsc::UnboundedSender<Job>,
}

struct InclusionServiceConfig {
    da_node_token: String,
    da_node_ws: String,
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

        // Check DB for finished jobs
        if let Some(proof_data) = self
            .0
            .finished_db
            .get(&job_key)
            .map_err(|e| Status::internal(e.to_string()))?
        {
            debug!("Job is finished, returning status");
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
                    let e = "Finished DB is in invalid state";
                    error!("{e}");
                    return Err(Status::internal(e));
                }
            }
        }

        // Check DB for pending jobs
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

        // No jobs in DB, create a new one
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
                        let da_client_handle = self.get_da_client().await.clone();
                        self.get_zk_proof_input_from_da(&job, job_key, da_client_handle)
                            .await?
                    }
                    JobStatus::DataAvalibile(proof_input) => {
                        // TODO handle non-hardcoded ZK programs
                        let program_id: SuccNetProgramId = *KECCAK_INCLUSION_ID
                            .get_or_init(|| async { Sha3_256::digest(KECCAK_INCLUSION_ELF).into() })
                            .await;
                        match self.request_zk_proof(program_id, proof_input).await {
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

    /// Given a SHA3 hash of a ZK program, get the require setup.
    /// The setup is a very heavy task and produces a large output (~200MB),
    /// fortunately it's identical per ZK program, so we store this in a DB to recall it.
    /// We load it and return a pointer to a single instance of this large setup object
    /// to read from for many concurrent [Job]s.
    async fn get_proof_setup(
        &self,
        zk_program_elf_sha3: [u8; 32],
        zk_client_handle: Arc<SP1NetworkProver>,
    ) -> Result<Arc<SP1ProofSetup>, InclusionServiceError> {
        debug!("Getting prover setup");
        let setup = KECCAK_INCLUSION_SETUP
            .get_or_try_init(|| async {
                // Check DB for existing pre-computed setup
                let precomputed_proof_setup = self
                    .config_db
                    .get(zk_program_elf_sha3)
                    .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

                let proof_setup = if let Some(precomputed) = precomputed_proof_setup {
                    bincode::deserialize(&precomputed)
                        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?
                } else {
                    info!(
                "No ZK proof setup in DB for SHA3_256 = 0x{} -- generation & storing in config DB",
                hex::encode(zk_program_elf_sha3)
            );

                    let new_proof_setup: SP1ProofSetup = tokio::task::spawn_blocking(move || {
                        zk_client_handle.setup(KECCAK_INCLUSION_ELF).into()
                    })
                    .await
                    .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

                    self.config_db
                        .insert(
                            &zk_program_elf_sha3,
                            bincode::serialize(&new_proof_setup)
                                .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?,
                        )
                        .map_err(|e| InclusionServiceError::GeneralError(e.to_string()))?;

                    new_proof_setup
                };
                Ok(Arc::new(proof_setup))
            })
            .await?
            .clone();

        Ok(setup)
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
                InclusionServiceError::DaClientError(e.to_string())
            })?;

        let nmt_multiproofs = client
            .blob_get_proof(job.height.into(), job.namespace, job.commitment)
            .await
            .map_err(|e| {
                error!("Failed to get blob proof from Celestia: {}", e);
                InclusionServiceError::DaClientError(e.to_string())
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
        Err(InclusionServiceError::DaClientError(format!(
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
                    e = InclusionServiceError::DaClientError("header: not found. Likely Celestia Node is missing blob, although it exists on the network overall.".to_string());
                    error!("{job:?} failed, recoverable: {e}");
                    job_status = JobStatus::Failed(
                        e.clone(),
                        Some(JobStatus::DataAvalibilityPending.into()),
                    );
                } else if error_object.message() == "blob: not found" {
                    e = InclusionServiceError::DaClientError("blob: not found".to_string());
                    error!("{job:?} failed, not recoverable: {e}");
                    job_status = JobStatus::Failed(e.clone(), None);
                } else {
                    e = InclusionServiceError::DaClientError("UNKNOWN client error".to_string());
                    error!("{job:?} failed, not recoverable: {e}");
                    job_status = JobStatus::Failed(e.clone(), None);
                }
            }
            // TODO: handle other Celestia JSON RPC errors
            _ => {
                e = InclusionServiceError::DaClientError(
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
        program_id: SuccNetProgramId,
        proof_input: KeccakInclusionToDataRootProofInput,
    ) -> Result<SuccNetJobId, InclusionServiceError> {
        debug!("Preparing prover network request and starting proving");
        let zk_client_handle = self.get_zk_client_remote().await;
        let proof_setup = self
            .get_proof_setup(program_id, zk_client_handle.clone())
            .await?;

        let mut stdin = SP1Stdin::new();
        stdin.write(&proof_input);
        let request_id: SuccNetJobId = zk_client_handle
            .prove(&proof_setup.pk, &stdin)
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
        let zk_client_handle = self.get_zk_client_remote().await;

        zk_client_handle
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
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                queue_tx.remove(job_key.clone())?;
                finished_tx.insert(job_key.clone(), bincode::serialize(&job_status).unwrap())?;
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

    async fn get_da_client(&self) -> Arc<CelestiaJSONClient> {
        let handle = self
            .da_client_handle
            .get_or_init(|| async {
                debug!("Building DA client");
                let client = CelestiaJSONClient::new(
                    self.config.da_node_ws.as_str(),
                    self.config.da_node_token.as_str().into(),
                )
                .await
                .expect("Failed to build Celestia Client RPC");
                Arc::new(client)
            })
            .await
            .clone();
        handle
    }

    async fn get_zk_client_remote(&self) -> Arc<SP1NetworkProver> {
        let handle = self
            .zk_client_handle
            .get_or_init(|| async {
                debug!("Building prover client");
                let client = sp1_sdk::ProverClient::builder().network().build();
                Arc::new(client)
            })
            .await
            .clone();
        handle
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    std::env::var("NETWORK_PRIVATE_KEY")
        .expect("NETWORK_PRIVATE_KEY for Succinct Prover env var reqired");
    let da_node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")
        .expect("CELESTIA_NODE_AUTH_TOKEN env var required");
    let da_node_ws = std::env::var("CELESTIA_NODE_WS").expect("CELESTIA_NODE_WS env var required");
    let db_path = std::env::var("EQ_DB_PATH").expect("EQ_DB_PATH env var required");
    let service_socket: std::net::SocketAddr = std::env::var("EQ_SOCKET")
        .expect("EQ_SOCKET env var required")
        .parse()
        .expect("EQ_SOCKET env var reqired");

    let db = sled::open(db_path.clone())?;
    let queue_db = db.open_tree("queue")?;
    let finished_db = db.open_tree("finished")?;
    let config_db = db.open_tree("config")?;

    info!("Running preflight setup");
    let (job_sender, job_receiver) = mpsc::unbounded_channel::<Job>();
    let inclusion_service = Arc::new(InclusionService {
        config: InclusionServiceConfig {
            da_node_token,
            da_node_ws,
        },
        da_client_handle: OnceCell::new(),
        zk_client_handle: OnceCell::new(),
        config_db: config_db.clone(),
        queue_db: queue_db.clone(),
        finished_db: finished_db.clone(),
        job_sender: job_sender.clone(),
    });

    tokio::spawn({
        let service = inclusion_service.clone();
        async move {
            service.clone().get_zk_client_remote().await;
            debug!("ZK client ready!");
        }
        // TODO: crash whole program if this fails
    });

    debug!("Starting service");
    tokio::spawn({
        let service = inclusion_service.clone();
        async move { service.job_worker(job_receiver).await }
    });

    tokio::spawn({
        let service = inclusion_service.clone();
        async move {
            service.clone().get_da_client().await;
            debug!("DA client ready!");
        }
        // TODO: crash whole program if this fails
    });

    debug!("Restarting unfinised jobs");
    for entry_result in queue_db.iter() {
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
