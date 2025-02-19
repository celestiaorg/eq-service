use eq_common::eqs::inclusion_client::InclusionClient;
use eq_common::eqs::{GetKeccakInclusionRequest, GetKeccakInclusionResponse};
use tonic::transport::Channel;
use tonic::Status as TonicStatus;

pub mod types;
use types::BlobId;

#[derive(Debug)]
pub struct EqClient {
    grpc_channel: Channel,
}

pub trait EqInterface {
    fn get_channel(&self) -> &Channel;

    fn get_keccak_inclusion(
        &self,
        request: &BlobId,
    ) -> impl std::future::Future<Output = Result<GetKeccakInclusionResponse, TonicStatus>> + Send
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
            let mut client = InclusionClient::new(self.get_channel().clone());
            match client.get_keccak_inclusion(request).await {
                Ok(response) => Ok(response.into_inner()),
                Err(e) => Err(e),
            }
        }
    }
}

impl EqInterface for EqClient {
    fn get_channel(&self) -> &Channel {
        &self.grpc_channel
    }
}

impl EqClient {
    pub fn new(grpc_channel: Channel) -> Self {
        Self { grpc_channel }
    }
}
