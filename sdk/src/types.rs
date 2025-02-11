use std::{error::Error, fmt::Display, str::FromStr};

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

/// Format = "height:namespace:commitment" using u64 for height, and base64 encoding for namespace and commitment
impl Display for BlobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let namespace_string;
        if let Some(namespace) = &self.namespace.id_v0() {
            namespace_string = base64::engine::general_purpose::STANDARD.encode(namespace);
        } else {
            namespace_string = "Invalid v0 ID".to_string()
        }
        let commitment_string =
            base64::engine::general_purpose::STANDARD.encode(&self.commitment.hash());
        write!(
            f,
            "{}:{}:{}",
            self.height.value(),
            &namespace_string,
            &commitment_string
        )
    }
}

/// Format = "height:namespace:commitment" using u64 for height, and base64 encoding for namespace and commitment
impl FromStr for BlobId {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(3, ":");

        let height = BlockHeight::from_str(parts.next().ok_or("Height missing (u64)")?)?;

        let n_base64 = parts
            .next()
            .ok_or("Namespace missing (base64)")?
            .to_string();
        let n_bytes = base64::engine::general_purpose::STANDARD.decode(n_base64)?;
        let namespace = Namespace::new_v0(&n_bytes)?;

        let c_base64 = parts
            .next()
            .ok_or("Commitment missing (base64)")?
            .to_string();
        let c_bytes = base64::engine::general_purpose::STANDARD.decode(c_base64)?;
        let c_hash: [u8; 32] = c_bytes
            .try_into()
            .map_err(|_| "Commitment must be 32 bytes!")?;
        let commitment = Commitment::new(c_hash.into());

        Ok(Self {
            height,
            namespace,
            commitment,
        })
    }
}
