use async_trait::async_trait;
use zksync_da_clients::{
    types::{DAError, DispatchResponse, InclusionData},
    DataAvailabilityClient,
};

#[derive(Debug, Clone)]
struct EqClient;

#[async_trait]
impl DataAvailabilityClient for EqClient {
    async fn dispatch_blob(
        &self,
        batch_number: u32,
        data: Vec<u8>,
    ) -> Result<DispatchResponse, DAError> {
        todo!()
    }

    async fn get_inclusion_data(&self, blob_id: &str) -> Result<Option<InclusionData>, DAError> {
        todo!()
    }

    /// Clones the client and wraps it in a Box.
    fn clone_boxed(&self) -> Box<dyn DataAvailabilityClient> {
        todo!()
    }

    /// Returns the maximum size of the blob (in bytes) that can be dispatched. None means no limit.
    fn blob_size_limit(&self) -> Option<usize> {
        todo!()
    }

    async fn balance(&self) -> Result<u64, DAError> {
        todo!()
    }
}

fn main() {}
