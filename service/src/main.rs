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

use eq_common::create_inclusion_proof_input;
use serde::{Deserialize, Serialize};

const KECCAK_INCLUSION_ELF: &[u8] = include_bytes!(
    "../../target/release/eq-program-keccak-inclusion"
);

#[derive(Serialize, Deserialize)]
pub struct Job {
    pub height: u64,
    pub namespace: Vec<u8>,
    pub commitment: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    // The Succinct Network job ID
    Pending(String),
    // For now we'll use the SP1ProofWithPublicValues as the proof
    // Ideally we only want the public values + whatever is needed to verify the proof
    // They don't seem to provide a type for that.
    Completed(SP1ProofWithPublicValues),
    Failed(String),
}
pub struct InclusionService {
    client: Arc<Client>,
    db: sled::Db,
}

#[tonic::async_trait]
impl Inclusion for InclusionService {
    async fn get_keccak_inclusion(
        &self,
        request: Request<GetKeccakInclusionRequest>,
    ) -> Result<Response<GetKeccakInclusionResponse>, Status> {
        let request = request.into_inner();

        let job_from_db = self
            .db
            .get(
                &bincode::serialize(&Job {
                    height: request.height,
                    namespace: request.namespace.clone(),
                    commitment: request.commitment.clone(),
                })
                .map_err(|e| Status::internal(e.to_string()))?,
            )
            .map_err(|e| Status::internal(e.to_string()))?;

        if let Some(job) = job_from_db {
            let job: JobStatus =
                bincode::deserialize(&job).map_err(|e| Status::internal(e.to_string()))?;
            match job {
                JobStatus::Pending(job_id) => {
                    return Ok(Response::new(GetKeccakInclusionResponse {
                        status: ResponseStatus::Waiting as i32,
                        response_value: Some(ResponseValue::ProofId(job_id)),
                    }));
                }
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
            };
        }

        let height = request.height;
        let commitment = Commitment::new(
            request
                .commitment
                .try_into()
                .map_err(|_| Status::invalid_argument("Invalid commitment"))?,
        );
        let namespace = Namespace::from_raw(&request.namespace)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let blob = self
            .client
            .blob_get(height, namespace, commitment)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Get the ExtendedHeader
        let header = self
            .client
            .header_get_by_height(height)
            .await
            .map_err(|e| Status::internal(format!("Failed to get header: {}", e.to_string())))?;

        let nmt_multiproofs = self
            .client
            .blob_get_proof(height, namespace, commitment)
            .await
            .map_err(|e| {
                Status::internal(format!("Failed to get blob proof: {}", e.to_string()))
            })?;

        let inclusion_proof_input = create_inclusion_proof_input(&blob, &header, nmt_multiproofs)
            .map_err(|e| Status::internal(e.to_string()))?;

        let network_prover = ProverClient::builder().network().build();
        let (pk, vk) = network_prover.setup(KECCAK_INCLUSION_ELF);

        let mut stdin = SP1Stdin::new();
        stdin.write(&inclusion_proof_input);
        let request_id = network_prover
            .prove(&pk, &stdin)
            .groth16()
            .request_async()
            .await
            .unwrap(); // TODO: Handle this error

        // TODO: Write a pending job to the DB, and start a worker to wait for the proof and update DB when it's complete
        // do so in a way so the service can recover from crashes, remember which jobs it's already started, and update DB when they're finished

        Ok(Response::new(GetKeccakInclusionResponse {
            status: 0,
            response_value: None,
        }))
    }
}

        Ok(Response::new(GetKeccakInclusionResponse {
            status: 0,
            response_value: None,
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let db = sled::open(args.db_path)?;

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("Token not provided");
    let client = Client::new("ws://localhost:26658", Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let addr = "[::1]:50051".parse()?;
    let inclusion_service = InclusionService {
        client: Arc::new(client),
        db: db,
    };

    Server::builder()
        .add_service(InclusionServer::new(inclusion_service))
        .serve(addr)
        .await?;

    Ok(())
}
