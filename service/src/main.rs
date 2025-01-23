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
use celestia_types::nmt::{Namespace, NamespacedHashExt};
use clap::Parser;
use nmt_rs::{
    simple_merkle::{
        db::MemDb,
        proof::Proof,
        tree::{MerkleHash, MerkleTree},
    },
    TmSha2Hasher,
};
use sp1_sdk::{NetworkProver, Prover, ProverClient, SP1Proof, SP1ProofWithPublicValues, SP1Stdin};
use std::cmp::max;
use tendermint::{hash::Algorithm, Hash as TmHash};
use tendermint_proto::{
    v0_37::{types::BlockId as RawBlockId, version::Consensus as RawConsensusVersion},
    Protobuf,
};
use tokio::sync::mpsc;

use eq_common::{KeccakInclusionToDataRootProofInput, create_inclusion_proof_input};
use serde::{Serialize, Deserialize};
use sled::Tree as SledTree;
use log::{debug, error, log_enabled, info, Level};

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!("../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/eq-program-keccak-inclusion");
type SuccNetJobId = [u8; 32];

#[derive(Serialize, Deserialize)]
pub struct Job {
    pub height: u64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum JobStatus {
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
    job_sender: mpsc::UnboundedSender<(SuccNetJobId, Job)>,
    proof_sender: mpsc::UnboundedSender<(Job, SP1ProofWithPublicValues)>,
    queue_tree: SledTree,
    proof_tree: SledTree,
}

#[tonic::async_trait]
impl Inclusion for InclusionService {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        let request = request.into_inner();
        info!("Received grpc request for commitment: {}", hex::encode(request.commitment.clone()));
        let job = Job {
            height: request.height,
            namespace: request.namespace.clone(),
            commitment: request.commitment.clone(),
        };
        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // First check proof_tree for completed/failed proofs
        debug!("Checking proof_tree for finished/failed proofs");
        if let Some(proof_data) = self.proof_tree.get(&job_key).map_err(|e| Status::internal(e.to_string()))? {
            let job_status: JobStatus = bincode::deserialize(&proof_data)
                .map_err(|e| Status::internal(e.to_string()))?;
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
        if let Some(queue_data) = self.queue_tree.get(&job_key).map_err(|e| Status::internal(e.to_string()))? {
            let job_status: JobStatus = bincode::deserialize(&queue_data)
                .map_err(|e| Status::internal(e.to_string()))?;
            if let JobStatus::Pending(job_id) = job_status {
                return Ok(Response::new(GetKeccakInclusionResponse { 
                    status: ResponseStatus::Waiting as i32, 
                    response_value: Some(ResponseValue::ProofId(job_id.to_vec()))
                }));
            }
        }

        // If not found in either tree, start new proof generation
        debug!("Preparing request to Celestia...");
        let height = request.height;
        let commitment = Commitment::new(
            request.commitment
            .clone()
            .try_into()
            .map_err(|_| Status::invalid_argument("Invalid commitment"))?
        );
        /*let namespace = Namespace::from_raw(&request.namespace)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;*/
        let namespace = Namespace::new_v0(&request.namespace)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        debug!("Getting blob from Celestia...");
        let blob = self.client.blob_get(height, namespace, commitment).await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Get the ExtendedHeader
        debug!("Getting header from Celestia...");
        let header = self.client.header_get_by_height(height)
            .await
            .map_err(|e| Status::internal(format!("Failed to get header: {}", e.to_string())))?;

        debug!("Getting NMT multiproofs from Celestia...");
        let nmt_multiproofs = self.client
            .blob_get_proof(height, namespace, commitment)
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to get blob proof: {}", e.to_string()))
            })?;

        debug!("Preparing prover network request and starting proving...");
        let inclusion_proof_input = create_inclusion_proof_input(&blob, &header, nmt_multiproofs)
            .map_err(|e| Status::internal(e.to_string()))?;

        let network_prover = ProverClient::builder().network().build();
        let (pk, vk) = network_prover.setup(KECCAK_INCLUSION_ELF);

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
        let serialized_status = bincode::serialize(&JobStatus::Pending(request_id))
            .map_err(|e| Status::internal(e.to_string()))?;
        self.queue_tree.insert(&job_key, serialized_status)
            .map_err(|e| Status::internal(e.to_string()))?;
        
        debug!("Sending job to proof worker...");
        // Send both job_id and key to proof worker
        self.job_sender.send((request_id, job))
            .map_err(|e| Status::internal(e.to_string()))?;

        debug!("Returning response...");
        Ok(Response::new(GetKeccakInclusionResponse { 
            status: ResponseStatus::Waiting as i32, 
            response_value: Some(ResponseValue::ProofId(request_id.to_vec()))
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

impl InclusionService {
    async fn job_worker(&self, mut job_receiver: mpsc::UnboundedReceiver<(SuccNetJobId, Job)>) {
        info!("Job worker started");
        while let Some((succnet_job_id, job)) = job_receiver.recv().await {
            debug!("job worker received succnet job id: {:?}", succnet_job_id);
            tokio::spawn(wait_for_proof(succnet_job_id, job, self.proof_sender.clone()));
        }
    }

    async fn db_worker(&self, mut proof_receiver: mpsc::UnboundedReceiver<(Job, SP1ProofWithPublicValues)>) {
        info!("Proof worker started");
        while let Some((job, proof)) = proof_receiver.recv().await {
            debug!("DB worker received completed proof for commitment: {}", hex::encode(job.commitment.clone()));
            let job_key = match bincode::serialize(&job) {
                Ok(key) => key,
                Err(e) => {
                    error!("Failed to serialize job: {}", e);
                    continue;
                }
            };
            match bincode::serialize(&JobStatus::Completed(proof)) {
                Ok(serialized_proof) => {
                    if let Err(e) = self.proof_tree.insert(&job_key, serialized_proof) {
                        error!("Failed to store proof in tree: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to serialize proof: {}", e);
                }
            }
            // Remove the job from the queue after processing
            if let Err(e) = self.queue_tree.remove(&job_key) {
                error!("Failed to remove job from queue: {}", e);
            }

        }
    }

}

async fn wait_for_proof(succnet_job_id: SuccNetJobId, job: Job, proof_sender: mpsc::UnboundedSender<(Job, SP1ProofWithPublicValues)>) {
    let network_prover = ProverClient::builder().network().build();
    match network_prover.wait_proof(succnet_job_id.into(), None).await {
        Ok(proof) => {
            debug!("Finished waiting for proof from succnet: {:?}", succnet_job_id);
            if let Err(e) = proof_sender.send((job, proof)) {
                error!("Failed to send proof: {}", e);
            }
        }
        Err(e) => {
            error!("Error waiting for proof: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();
    let db = sled::open(args.db_path)?;
    let queue_tree = db.open_tree("queue")?;
    let proof_tree = db.open_tree("proof")?;

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("Token not provided");
    let client = Client::new("ws://localhost:26658", Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let (job_sender, job_receiver) = mpsc::unbounded_channel::<(SuccNetJobId, Job)>();
    let (proof_sender, proof_receiver) = mpsc::unbounded_channel::<(Job, SP1ProofWithPublicValues)>();
    let inclusion_service = InclusionService{
        client: Arc::new(client),
        queue_tree: queue_tree.clone(),
        proof_tree: proof_tree.clone(),
        job_sender: job_sender.clone(),
        proof_sender: proof_sender.clone(),
    };

    let inclusion_service = Arc::new(inclusion_service);

    tokio::spawn({
        let service = Arc::clone(&inclusion_service);
        async move { service.job_worker(job_receiver).await }
    });
    tokio::spawn({
        let service = Arc::clone(&inclusion_service);
        async move { service.db_worker(proof_receiver).await }
    });

    let mut jobs_sent_on_startup = 0;
    // Process any existing jobs in the queue
    for entry_result in queue_tree.iter() {
        if let Ok((job_key, queue_data)) = entry_result {
            let job: Job = bincode::deserialize(&job_key).unwrap();
            if let Ok(job_status) = bincode::deserialize::<JobStatus>(&queue_data) {
                if let JobStatus::Pending(job_id) = job_status {
                    if let Err(e) = job_sender.send((job_id, job)) {
                        error!("Failed to send existing job to worker: {}", e);
                    } else {
                        jobs_sent_on_startup += 1;
                    }
                }
            }
        }
    }

    info!("Sent {} jobs on startup", jobs_sent_on_startup);

    let addr = "[::1]:50051".parse()?;

    Server::builder()
        .add_service(InclusionServer::new(Arc::clone(&inclusion_service)))
        .serve(addr)
        .await?;

    Ok(())
}
