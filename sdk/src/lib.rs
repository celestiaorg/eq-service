// Re-export eq-common parts
pub use eq_common::eqs::inclusion_client::InclusionClient;
pub use eq_common::eqs::{
    get_keccak_inclusion_response, GetKeccakInclusionRequest, GetKeccakInclusionResponse,
};
pub use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};

use tonic::transport::Channel;
use tonic::Status as TonicStatus;

pub mod types;
pub use types::BlobId;

#[derive(Debug)]
pub struct EqClient {
    grpc_channel: Channel,
}

impl EqClient {
    pub fn new(grpc_channel: Channel) -> Self {
        Self { grpc_channel }
    }
    pub fn get_keccak_inclusion<'a>(
        &'a self,
        request: &'a BlobId,
    ) -> impl std::future::Future<Output = Result<GetKeccakInclusionResponse, TonicStatus>> + Send + 'a
    where
        Self: Sync,
    {
        async {
            let request = GetKeccakInclusionRequest {
                commitment: request.commitment.hash().to_vec(),
                namespace: request
                    .namespace
                    .id_v0()
                    .ok_or(TonicStatus::invalid_argument("Namespace invalid"))?
                    .to_vec(),
                height: request.height.into(),
            };
            let mut client = InclusionClient::new(self.grpc_channel.clone());
            match client.get_keccak_inclusion(request).await {
                Ok(response) => Ok(response.into_inner()),
                Err(e) => Err(e),
            }
        }
    }
}
