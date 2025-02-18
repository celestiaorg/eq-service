use crate::{Job, JobStatus, SP1ProofSetup, SuccNetJobId, SuccNetProgramId};

use celestia_rpc::{BlobClient, Client as CelestiaJSONClient, HeaderClient, ShareClient};
use eq_common::{InclusionServiceError, KeccakInclusionToDataRootProofInput};
use jsonrpsee::core::ClientError as JsonRpcError;
use log::{debug, error, info};
use sha3::Keccak256;
use sha3::{Digest, Sha3_256};
use sled::{Transactional, Tree as SledTree};
use sp1_sdk::{
    network::Error as SP1NetworkError, NetworkProver as SP1NetworkProver, Prover,
    SP1ProofWithPublicValues, SP1Stdin,
};
use std::sync::Arc;
use tokio::sync::{mpsc, OnceCell};

/// Hardcoded ELF binary for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion"
);
/// Hardcoded ID for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_ID: OnceCell<SuccNetProgramId> = OnceCell::const_new();

/// Given a hard coded ELF, get it's ID
/// TODO: generalize
pub async fn get_program_id() -> SuccNetProgramId {
    *KECCAK_INCLUSION_ID
        .get_or_init(|| async {
            debug!("Building Program ID");
            Sha3_256::digest(KECCAK_INCLUSION_ELF).into()
        })
        .await
}

/// Hardcoded setup for the crate `program-keccak-inclusion`
static KECCAK_INCLUSION_SETUP: OnceCell<Arc<SP1ProofSetup>> = OnceCell::const_new();

/// The main service, depends on external DA and ZK clients internally!
pub struct InclusionService {
    pub config: InclusionServiceConfig,
    da_client_handle: OnceCell<Arc<CelestiaJSONClient>>,
    zk_client_handle: OnceCell<Arc<SP1NetworkProver>>,
    pub config_db: SledTree,
    pub queue_db: SledTree,
    pub finished_db: SledTree,
    pub job_sender: mpsc::UnboundedSender<Option<Job>>,
}

impl InclusionService {
    pub fn new(
        config: InclusionServiceConfig,
        da_client_handle: OnceCell<Arc<CelestiaJSONClient>>,
        zk_client_handle: OnceCell<Arc<SP1NetworkProver>>,
        config_db: SledTree,
        queue_db: SledTree,
        finished_db: SledTree,
        job_sender: mpsc::UnboundedSender<Option<Job>>,
    ) -> Self {
        InclusionService {
            config,
            da_client_handle,
            zk_client_handle,
            config_db,
            queue_db,
            finished_db,
            job_sender,
        }
    }
}

pub struct InclusionServiceConfig {
    pub da_node_token: String,
    pub da_node_ws: String,
}

impl InclusionService {
    /// A worker that receives [Job]s by a channel and drives them to completion.
    ///
    /// `Job`s are complete stepwise with a [JobStatus] that is recorded in a
    /// queue database, with work to progress each status to towards completion async.
    /// Once a step is completed, `JobStatus` is recorded into the queue database that
    /// recursively, driving to `Job` completion.
    ///
    /// When a successful or failed state is arrived at,
    /// the job is atomically removed from the queue and added to a results database.
    pub async fn job_worker(
        self: Arc<Self>,
        mut job_receiver: mpsc::UnboundedReceiver<Option<Job>>,
    ) {
        debug!("Job worker started");
        while let Some(Some(job)) = job_receiver.recv().await {
            let service = self.clone();
            tokio::spawn(async move {
                debug!("Job worker received {job:?}",);
                let _ = service.prove(job).await; //Don't return with "?", we run keep looping
            });
        }

        info!("Shutting down");
        let _ = self.queue_db.flush();
        let _ = self.finished_db.flush();
        info!("Cleanup complete");

        std::process::exit(0);
    }

    /// The main service task: produce a proof based on a [Job] requested.
    pub async fn prove(&self, job: Job) -> Result<(), InclusionServiceError> {
        let job_key = bincode::serialize(&job).unwrap();
        if let Some(queue_data) = self.queue_db.get(&job_key).unwrap() {
            let mut job_status: JobStatus = bincode::deserialize(&queue_data).unwrap();
            debug!("Job worker processing with starting status: {job_status:?}");
            match job_status {
                JobStatus::DataAvailabilityPending => {
                    let da_client_handle = self.get_da_client().await.clone();
                    self.get_zk_proof_input_from_da(&job, &job_key, da_client_handle?)
                        .await?;
                    debug!("DA data -> zk input ready");
                }
                JobStatus::DataAvailable(proof_input) => {
                    // TODO handle non-hardcoded ZK programs
                    match self
                        .request_zk_proof(&get_program_id().await, &proof_input, &job, &job_key)
                        .await
                    {
                        Ok(zk_job_id) => {
                            job_status = JobStatus::ZkProofPending(zk_job_id);
                            self.send_job_with_new_status(job_key, job_status, job)?;
                        }
                        Err(e) => {
                            error!("{job:?} failed progressing DataAvailable: {e}");
                            job_status = JobStatus::Failed(
                                e,
                                Some(JobStatus::DataAvailable(proof_input).into()),
                            );
                            self.finalize_job(&job_key, job_status)?;
                        }
                    };
                    debug!("ZK request sent");
                }
                JobStatus::ZkProofPending(zk_request_id) => {
                    debug!("ZK request waiting");
                    match self.wait_for_zk_proof(&job_key, zk_request_id).await {
                        Ok(zk_proof) => {
                            info!("🎉 {job:?} Finished!");
                            job_status = JobStatus::ZkProofFinished(zk_proof);
                            self.finalize_job(&job_key, job_status)?;
                        }
                        Err(e) => {
                            error!("{job:?} failed progressing ZkProofPending: {e}");
                            job_status = JobStatus::Failed(
                                e,
                                Some(JobStatus::ZkProofPending(zk_request_id).into()),
                            );
                            self.finalize_job(&job_key, job_status)?;
                        }
                    }
                    debug!("ZK request fulfilled");
                }
                _ => error!("Queue has INVALID status! Finished jobs stuck in queue!"),
            }
        };
        Ok(())
    }

    /// Given a SHA3 hash of a ZK program, get the require setup.
    /// The setup is a very heavy task and produces a large output (~200MB),
    /// fortunately it's identical per ZK program, so we store this in a DB to recall it.
    /// We load it and return a pointer to a single instance of this large setup object
    /// to read from for many concurrent [Job]s.
    pub async fn get_proof_setup(
        &self,
        zk_program_elf_sha3: &[u8; 32],
        zk_client_handle: Arc<SP1NetworkProver>,
    ) -> Result<Arc<SP1ProofSetup>, InclusionServiceError> {
        debug!("Getting ZK program proof setup");
        let setup = KECCAK_INCLUSION_SETUP
            .get_or_try_init(|| async {
                // Check DB for existing pre-computed setup
                let precomputed_proof_setup = self
                    .config_db
                    .get(zk_program_elf_sha3)
                    .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?;

                let proof_setup = if let Some(precomputed) = precomputed_proof_setup {
                    bincode::deserialize(&precomputed)
                        .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?
                } else {
                    info!(
                        "No ZK proof setup in DB for SHA3_256 = 0x{} -- generation & storing in config DB",
                        hex::encode(zk_program_elf_sha3)
                    );

                    let new_proof_setup: SP1ProofSetup = tokio::task::spawn_blocking(move || {
                        zk_client_handle.setup(KECCAK_INCLUSION_ELF).into()
                    })
                    .await
                    .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?;

                    self.config_db
                        .insert(
                            zk_program_elf_sha3,
                            bincode::serialize(&new_proof_setup)
                                .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?,
                        )
                        .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?;

                    new_proof_setup
                };
                Ok(Arc::new(proof_setup))
            })
            .await?
            .clone();

        Ok(setup)
    }

    /// Connects to a [CelestiaJSONClient] and attempts to get a inclusion proof for a [Job].
    /// On `Ok(())`, the queue DB contains valid ZKP input inside a new [JobStatus::DataAvailable] on the queue.
    async fn get_zk_proof_input_from_da(
        &self,
        job: &Job,
        job_key: &[u8],
        client: Arc<CelestiaJSONClient>,
    ) -> Result<(), InclusionServiceError> {
        debug!("Preparing request to Celestia");

        let header = client
            .header_get_by_height(job.height.into())
            .await
            .map_err(|e| self.handle_da_client_error(e, job, job_key))?;

        let eds_row_roots = header.dah.row_roots();
        let eds_size: u64 = eds_row_roots.len().try_into().unwrap();
        let ods_size: u64 = eds_size / 2;

        let blob = client
            .blob_get(job.height.into(), job.namespace, job.commitment)
            .await
            .map_err(|e| self.handle_da_client_error(e, job, job_key))?;

        let blob_index = blob
            .index
            .ok_or_else(|| InclusionServiceError::MissingBlobIndex)?;
        let first_row_index: u64 = blob_index.div_ceil(eds_size) - 1;
        let ods_index = blob_index - (first_row_index * ods_size);

        let range_response = client
            .share_get_range(&header, ods_index, ods_index + blob.shares_len() as u64)
            .await
            .map_err(|e| self.handle_da_client_error(e, job, job_key))?;

        range_response
            .proof
            .verify(header.dah.hash())
            .map_err(|_| InclusionServiceError::FailedShareRangeProofSanityCheck)?;

        let keccak_hash: [u8; 32] = Keccak256::new().chain_update(&blob.data).finalize().into();

        debug!("Creating ZK Proof input from Celestia Data");
        let proof_input = KeccakInclusionToDataRootProofInput {
            data: blob.data,
            namespace_id: job.namespace,
            share_proofs: range_response.proof.share_proofs,
            row_proof: range_response.proof.row_proof,
            data_root: header.dah.hash().as_bytes().try_into().unwrap(),
            keccak_hash: keccak_hash,
        };

        self.send_job_with_new_status(
            job_key.to_vec(),
            JobStatus::DataAvailable(proof_input),
            job.clone(),
        )
    }

    /// Helper function to handle error from a [jsonrpsee] based DA client.
    /// Will finalize the job in an [JobStatus::Failed] state,
    /// that may be retryable.
    fn handle_da_client_error(
        &self,
        da_client_error: JsonRpcError,
        job: &Job,
        job_key: &[u8],
    ) -> InclusionServiceError {
        error!("Celestia Client error: {da_client_error}");
        let (e, job_status);
        let call_err = "DA Call Error: ".to_string() + &da_client_error.to_string();
        match da_client_error {
            JsonRpcError::Call(error_object) => {
                // TODO: make this handle errors much better! JSON stringiness is a problem!
                if error_object.message().starts_with("header: not found") {
                    e = InclusionServiceError::DaClientError(format!("{call_err} - Likely DA Node is not properly synced, and blob does exists on the network. PLEASE REPORT!"));
                    job_status = JobStatus::Failed(
                        e.clone(),
                        Some(JobStatus::DataAvailabilityPending.into()),
                    );
                } else if error_object
                    .message()
                    .starts_with("header: given height is from the future")
                {
                    e = InclusionServiceError::DaClientError(format!("{call_err}"));
                    job_status = JobStatus::Failed(e.clone(), None);
                } else if error_object
                    .message()
                    .starts_with("header: syncing in progress")
                {
                    e = InclusionServiceError::DaClientError(format!(
                        "{call_err} - Blob *may* exist on the network."
                    ));
                    job_status = JobStatus::Failed(
                        e.clone(),
                        Some(JobStatus::DataAvailabilityPending.into()),
                    );
                } else if error_object.message().starts_with("blob: not found") {
                    e = InclusionServiceError::DaClientError(format!(
                        "{call_err} - Likely incorrect request inputs."
                    ));
                    job_status = JobStatus::Failed(e.clone(), None);
                } else {
                    e = InclusionServiceError::DaClientError(format!(
                        "{call_err} - UNKNOWN DA client error. PLEASE REPORT!"
                    ));
                    job_status = JobStatus::Failed(e.clone(), None);
                }
            }
            JsonRpcError::RequestTimeout
            | JsonRpcError::Transport(_)
            | JsonRpcError::RestartNeeded(_) => {
                e = InclusionServiceError::DaClientError(format!("{da_client_error}"));
                job_status =
                    JobStatus::Failed(e.clone(), Some(JobStatus::DataAvailabilityPending.into()));
            }
            // TODO: handle other Celestia JSON RPC errors
            _ => {
                e = InclusionServiceError::DaClientError(
                    "Unhandled Celestia SDK error. PLEASE REPORT!".to_string(),
                );
                error!("{job:?} failed, not recoverable: {e}");
                job_status = JobStatus::Failed(e.clone(), None);
            }
        };
        match self.finalize_job(job_key, job_status) {
            Ok(_) => e,
            Err(internal_err) => internal_err,
        }
    }

    /// Helper function to handle error from a SP1 NetworkProver Clents.
    /// Will finalize the job in an [JobStatus::Failed] state,
    /// that may be retryable.
    fn handle_zk_client_error(
        &self,
        zk_client_error: &SP1NetworkError,
        job: &Job,
        job_key: &[u8],
    ) -> InclusionServiceError {
        error!("SP1 Client error: {zk_client_error}");
        let (e, job_status);
        match zk_client_error {
            SP1NetworkError::SimulationFailed | SP1NetworkError::RequestUnexecutable { .. } => {
                e = InclusionServiceError::DaClientError(
                    format!("ZKP program critical failure: {zk_client_error} occured for {job:?} PLEASE REPORT!"),
                );
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestUnfulfillable { .. } => {
                e = InclusionServiceError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occured for {job:?} PLEASE REPORT!"
                ));
                job_status = JobStatus::Failed(e.clone(), None);
            }
            SP1NetworkError::RequestTimedOut { request_id } => {
                e = InclusionServiceError::DaClientError(format!(
                    "ZKP network: {zk_client_error} occured for {job:?}"
                ));

                let id = request_id
                    .as_slice()
                    .try_into()
                    .expect("request ID is always correct length");
                job_status =
                    JobStatus::Failed(e.clone(), Some(JobStatus::ZkProofPending(id).into()));
            }
            SP1NetworkError::RpcError(_) | SP1NetworkError::Other(_) => {
                e = InclusionServiceError::DaClientError(format!(
                    "ZKP network failure: {zk_client_error} occured for {job:?} PLEASE REPORT!"
                ));
                // TODO: We cannot clone KeccakInclusionToDataRootProofInput thus we cannot insert into a JobStatus::DataAvalibile(proof_input)
                // So we just redo the work from scratch for the DA side as a stupid workaround
                job_status =
                    JobStatus::Failed(e.clone(), Some(JobStatus::DataAvailabilityPending.into()));
            }
        }
        match self.finalize_job(job_key, job_status) {
            Ok(_) => e,
            Err(internal_err) => internal_err,
        }
    }

    /// Start a proof request from Succinct's prover network
    pub async fn request_zk_proof(
        &self,
        program_id: &SuccNetProgramId,
        proof_input: &KeccakInclusionToDataRootProofInput,
        job: &Job,
        job_key: &[u8],
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
            .skip_simulation(false)
            .request_async()
            .await
            // TODO: how to handle errors without a concrete type? Anyhow is not the right thing for us...
            .map_err(|e| {
                if let Some(down) = e.downcast_ref::<SP1NetworkError>() {
                    return self.handle_zk_client_error(down, job, job_key);
                }
                InclusionServiceError::ZkClientError(format!("Unhandled Error: {e} PLEASE REPORT"))
            })?
            .into();

        Ok(request_id)
    }

    /// Await a proof request from Succinct's prover network
    async fn wait_for_zk_proof(
        &self,
        job_key: &[u8],
        request_id: SuccNetJobId,
    ) -> Result<SP1ProofWithPublicValues, InclusionServiceError> {
        debug!("Waiting for proof from prover network");
        let zk_client_handle = self.get_zk_client_remote().await;

        let proof = zk_client_handle
            .wait_proof(request_id.into(), None)
            .await
            .map_err(|e| {
                error!("UNHANDLED ZK client error: {e:?}");
                let e = InclusionServiceError::ZkClientError(
                    "UNKNOWN ZK client error. PLEASE REPORT!".to_string(),
                );
                match self.finalize_job(
                    job_key,
                    JobStatus::Failed(
                        e.clone(),
                        Some(JobStatus::ZkProofPending(request_id).into()),
                    ),
                ) {
                    Ok(_) => e,
                    Err(internal_err) => internal_err,
                }
            })?;
        Ok(proof)
    }

    /// Atomically move a job from the database queue tree to the proof tree.
    /// This removes the job from any further processing by workers.
    /// The [JobStatus] should be success or failure only
    /// (but this is not enforced or checked at this time)
    fn finalize_job(
        &self,
        job_key: &[u8],
        job_status: JobStatus,
    ) -> Result<(), InclusionServiceError> {
        // TODO: do we want to do a status check here? To prevent accidentally getting into a DB invalid state
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                queue_tx.remove(job_key)?;
                finished_tx.insert(
                    job_key,
                    bincode::serialize(&job_status).expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<InclusionServiceError>>(())
            })
            .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?;
        Ok(())
    }

    /// Insert a [JobStatus] into a [SledTree] database
    /// AND `send()` this job back to the `self.job_sender` to schedule more progress.
    /// You likely want to pass `self.some_sled_tree` into `data_base` as input.
    pub fn send_job_with_new_status(
        &self,
        job_key: Vec<u8>,
        update_status: JobStatus,
        job: Job,
    ) -> Result<(), InclusionServiceError> {
        debug!("Sending {job:?} back with updated status: {update_status:?}");
        (&self.queue_db, &self.finished_db)
            .transaction(|(queue_tx, finished_tx)| {
                finished_tx.remove(job_key.clone())?;
                queue_tx.insert(
                    job_key.clone(),
                    bincode::serialize(&update_status)
                        .expect("Always given serializable job status"),
                )?;
                Ok::<(), sled::transaction::ConflictableTransactionError<InclusionServiceError>>(())
            })
            .map_err(|e| InclusionServiceError::InternalError(e.to_string()))?;
        self.job_sender
            .send(Some(job))
            .map_err(|e| InclusionServiceError::InternalError(e.to_string()))
    }

    pub async fn get_da_client(&self) -> Result<Arc<CelestiaJSONClient>, InclusionServiceError> {
        let handle = self
            .da_client_handle
            .get_or_try_init(|| async {
                debug!("Building DA client");
                let client = CelestiaJSONClient::new(
                    self.config.da_node_ws.as_str(),
                    self.config.da_node_token.as_str().into(),
                )
                .await
                .map_err(|e| InclusionServiceError::DaClientError(e.to_string()))?;
                Ok(Arc::new(client))
            })
            .await
            .map_err(|e: InclusionServiceError| e)?;
        Ok(handle.clone())
    }

    pub async fn get_zk_client_remote(&self) -> Arc<SP1NetworkProver> {
        self.zk_client_handle
            .get_or_init(|| async {
                debug!("Building ZK client");
                let client = sp1_sdk::ProverClient::builder().network().build();
                Arc::new(client)
            })
            .await
            .clone()
    }

    pub fn shutdown(&self) {
        info!("Terminating worker,finishing prexisting jobs");
        let _ = self.job_sender.send(None); // Break loop in `job_worker`
    }
}
