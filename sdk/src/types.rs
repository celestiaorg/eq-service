use base64::Engine;
use celestia_types::{blob::Commitment, block::Height as BlockHeight, nmt::Namespace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct BlobId {
    pub height: BlockHeight,
    pub namespace: Namespace,
    pub commitment: Commitment,
}

impl BlobId {
    pub fn new(height: BlockHeight, namespace: Namespace, commitment: Commitment) -> Self {
        Self {
            height,
            namespace,
            commitment,
        }
    }
}

impl std::fmt::Debug for BlobId {
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
