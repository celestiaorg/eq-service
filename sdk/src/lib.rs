// Re-export eq-common parts
pub use eq_common::eqs::inclusion_client::InclusionClient;
pub use eq_common::eqs::{get_zk_stack_response, GetZkStackRequest, GetZkStackResponse};
pub use eq_common::{ZKStackEqProofInput, ZKStackEqProofOutput};

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
    pub fn get_zk_stack<'a>(
        &'a self,
        request: &'a BlobId,
    ) -> impl std::future::Future<Output = Result<GetZkStackResponse, TonicStatus>> + Send + 'a
    where
        Self: Sync,
    {
        async {
            let request = GetZkStackRequest {
                commitment: request.commitment.hash().to_vec(),
                namespace: request
                    .namespace
                    .id_v0()
                    .ok_or(TonicStatus::invalid_argument("Namespace invalid"))?
                    .to_vec(),
                height: request.height.into(),
                batch_number: request.batch_number,
                chain_id: request.l2_chain_id,
            };
            let mut client = InclusionClient::new(self.grpc_channel.clone());
            match client.get_zk_stack(request).await {
                Ok(response) => Ok(response.into_inner()),
                Err(e) => Err(e),
            }
        }
    }
}
