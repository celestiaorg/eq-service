use std::{error::Error, fmt::Display, str::FromStr};

use base64::Engine;
use celestia_types::{blob::Commitment, block::Height as BlockHeight, nmt::Namespace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct BlobId {
    pub height: BlockHeight,
    pub namespace: Namespace,
    pub commitment: Commitment,
    pub l2_chain_id: u64,
    pub batch_number: u32,
}

impl BlobId {
    pub fn new(height: BlockHeight, namespace: Namespace, commitment: Commitment, l2_chain_id: u64, batch_number: u32) -> Self {
        Self {
            height,
            namespace,
            commitment,
            l2_chain_id,
            batch_number,
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
            .field("l2_chain_id", &self.l2_chain_id)
            .field("batch_number", &self.batch_number)
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
            "{}:{}:{}:{}:{}",
            self.height.value(),
            &namespace_string,
            &commitment_string,
            self.l2_chain_id,
            self.batch_number
        )
    }
}

/// Format = "height:namespace:commitment" using u64 for height, and base64 encoding for namespace and commitment
impl FromStr for BlobId {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(4, ":");

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

        let batch_number = parts
            .next()
            .ok_or("Batch number missing (u32)")?
            .to_string();
        let batch_number = u32::from_str(&batch_number)?;

        let l2_chain_id = parts
            .next()
            .ok_or("L2 chain ID missing (u64)")?
            .to_string();
        let l2_chain_id = u64::from_str(&l2_chain_id)?;

        Ok(Self {
            height,
            namespace,
            commitment,
            l2_chain_id,
            batch_number,
        })
    }
}
