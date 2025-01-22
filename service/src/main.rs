use std::sync::Arc;
use tonic::{transport::Server, Request, Response, Status};

pub mod eqs {
    include!("generated/eqs.rs");
}
use eqs::inclusion_server::{Inclusion, InclusionServer};
use eqs::{GetKeccakInclusionRequest, GetKeccakInclusionResponse, get_keccak_inclusion_response::{ResponseValue, Status as ResponseStatus}};

use celestia_rpc::{BlobClient, Client, HeaderClient};
use celestia_types::nmt::{Namespace, NamespacedHashExt};
use celestia_types::blob::Commitment;
use tendermint::{hash::Algorithm, Hash as TmHash};
use tendermint_proto::{
    v0_37::{types::BlockId as RawBlockId, version::Consensus as RawConsensusVersion},
    Protobuf,
};
use std::cmp::max;
use clap::{Parser};
use nmt_rs::{
    simple_merkle::{db::MemDb, proof::Proof, tree::{MerkleTree, MerkleHash}},
    TmSha2Hasher,
};
use sp1_sdk::{ProverClient, SP1Proof, SP1ProofWithPublicValues, SP1Stdin, Prover, NetworkProver};
use tokio::sync::mpsc;

use eq_common::{KeccakInclusionToDataRootProofInput, create_inclusion_proof_input};
use serde::{Serialize, Deserialize};
use sled::Tree as SledTree;

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
    sender: mpsc::UnboundedSender<(SuccNetJobId, Vec<u8>)>,
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
        let job = Job {
            height: request.height,
            namespace: request.namespace.clone(),
            commitment: request.commitment.clone(),
        };
        let job_key = bincode::serialize(&job).map_err(|e| Status::internal(e.to_string()))?;

        // First check proof_tree for completed/failed proofs
        if let Some(proof_data) = self.proof_tree.get(&job_key).map_err(|e| Status::internal(e.to_string()))? {
            let job_status: JobStatus = bincode::deserialize(&proof_data)
                .map_err(|e| Status::internal(e.to_string()))?;
            match job_status {
                JobStatus::Completed(proof) => {
                    return Ok(Response::new(GetKeccakInclusionResponse { 
                        status: ResponseStatus::Complete as i32, 
                        response_value: Some(ResponseValue::Proof(bincode::serialize(&proof).map_err(|e| Status::internal(e.to_string()))?))
                    }));
                }
                JobStatus::Failed(error) => {
                    return Ok(Response::new(GetKeccakInclusionResponse { 
                        status: ResponseStatus::Failed as i32, 
                        response_value: Some(ResponseValue::ErrorMessage(error))
                    }));
                }
                _ => return Err(Status::internal("Invalid state in proof_tree")),
            }
        }

        // Then check queue_tree for pending proofs
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
        let height = request.height;
        let commitment = Commitment::new(
            request.commitment
            .clone()
            .try_into()
            .map_err(|_| Status::invalid_argument("Invalid commitment"))?
        );
        let namespace = Namespace::from_raw(&request.namespace)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let blob = self.client.blob_get(height, namespace, commitment).await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Get the ExtendedHeader
        let header = self.client.header_get_by_height(height)
            .await
            .map_err(|e| Status::internal(format!("Failed to get header: {}", e.to_string())))?;

        let nmt_multiproofs = self.client
            .blob_get_proof(height, namespace, commitment)
            .await
            .map_err(|e| Status::internal(format!("Failed to get blob proof: {}", e.to_string())))?;

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

        // Store in queue_tree
        let serialized_status = bincode::serialize(&JobStatus::Pending(request_id))
            .map_err(|e| Status::internal(e.to_string()))?;
        self.queue_tree.insert(&job_key, serialized_status)
            .map_err(|e| Status::internal(e.to_string()))?;
        
        // Send both job_id and key to worker
        self.sender.send((request_id, job_key))
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetKeccakInclusionResponse { 
            status: ResponseStatus::Waiting as i32, 
            response_value: Some(ResponseValue::ProofId(request_id.to_vec()))
        }))
    }

}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    db_path: String,
}

impl InclusionService {
    async fn worker(&self, mut receiver: mpsc::UnboundedReceiver<(SuccNetJobId, Vec<u8>)>) {
        println!("Worker started");
        while let Some((job_id, job_key)) = receiver.recv().await {
            println!("Received job id: {:?}", job_id);
            tokio::spawn(self.wait_for_proof(job_id, job_key));
        }
    }

    async fn wait_for_proof(&self, job_id: SuccNetJobId, job_key: Vec<u8>) {
        let network_prover = ProverClient::builder().network().build();
        
        match network_prover.wait_proof(job_id.into(), None).await {
            Ok(proof) => {
                println!("Proof received for job: {:?}", job_id);
                
                // Store the completed proof
                if let Ok(serialized_proof) = bincode::serialize(&JobStatus::Completed(proof)) {
                    if let Err(e) = self.proof_tree.insert(&job_key, serialized_proof) {
                        println!("Failed to store proof: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Error waiting for proof: {}", e);
                
                // Store the error
                if let Ok(serialized_error) = bincode::serialize(&JobStatus::Failed(e.to_string())) {
                    if let Err(e) = self.proof_tree.insert(&job_key, serialized_error) {
                        println!("Failed to store error: {}", e);
                    }
                }
            }
        }

        // Remove from queue regardless of success/failure
        if let Err(e) = self.queue_tree.remove(&job_key) {
            println!("Failed to remove from queue: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args = Args::parse();
    let db = sled::open(args.db_path)?;
    let queue_tree = db.open_tree("queue")?;
    let proof_tree = db.open_tree("proof")?;

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("Token not provided");
    let client = Client::new("ws://localhost:26658", Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let (sender, receiver) = mpsc::unbounded_channel::<(SuccNetJobId, Vec<u8>)>();

    tokio::spawn(worker(receiver));

    let addr = "[::1]:50051".parse()?;
    let inclusion_service = InclusionService{
        client: Arc::new(client),
        queue_tree,
        proof_tree,
        sender,
    };

    Server::builder()
        .add_service(InclusionServer::new(inclusion_service))
        .serve(addr)
        .await?;

    Ok(())
}