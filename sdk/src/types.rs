use std::{error::Error, fmt::Display, str::FromStr};

use base64::Engine;
use celestia_types::{blob::Commitment, block::Height as BlockHeight, nmt::Namespace};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct BlobId {
    pub height: BlockHeight,
    pub namespace: Namespace,
    pub commitment: Commitment,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct JobId {
    pub blob_id: BlobId,
    // Used to prevent replay of proofs for the same blob
    pub l2_chain_id: u64,
    pub batch_number: u32,
}

impl JobId {
    pub fn new(blob_id: BlobId, l2_chain_id: u64, batch_number: u32) -> Self {
        Self {
            blob_id,
            l2_chain_id,
            batch_number,
        }
    }
}

/// Format = "height:namespace:commitment:l2_chain_id:batch_number" using integers for height, l2_chain_id, and batch; base64 encoding for namespace and commitment
impl FromStr for JobId {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(3, ":");

        let blob_id = BlobId::from_str(parts.next().ok_or("BlobId missing")?)?;

        let l2_chain_id = parts.next().ok_or("L2 chain ID missing (u64)")?.to_string();
        let l2_chain_id = u64::from_str(&l2_chain_id)?;

        let batch_number = parts
            .next()
            .ok_or("Batch number missing (u32)")?
            .to_string();
        let batch_number = u32::from_str(&batch_number)?;

        Ok(Self {
            blob_id,
            l2_chain_id,
            batch_number,
        })
    }
}

/// Format = "height:namespace:commitment:l2_chain_id:batch_number" using integers for height, l2_chain_id, and batch; base64 encoding for namespace and commitment
impl Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}:",
            self.blob_id.to_string(),
            &self.l2_chain_id,
            &self.batch_number,
        )
    }
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

/// Format = "height:namespace:commitment:l2_chain_id:batch_number" using integers for height, l2_chain_id, and batch; base64 encoding for namespace and commitment
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
            "{}:{}:{}:",
            self.height.value(),
            &namespace_string,
            &commitment_string,
        )
    }
}

/// Format = "height:namespace:commitment:l2_chain_id:batch_number" using integers for height, l2_chain_id, and batch; base64 encoding for namespace and commitment
impl FromStr for BlobId {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(5, ":");

        let height = BlockHeight::from_str(parts.next().ok_or("Height missing (u64)")?)?;

        let n_base64 = parts
            .next()
            .ok_or("Namespace missing (base64)")?
            .to_string();
        println!("{}", &n_base64);
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

#[cfg(test)]
mod test {
    use base64::engine::general_purpose::STANDARD;

    use super::*;
    use bincode;

    #[test]
    fn test_job_id_from_str() {
        let height: u32 = 6952283;
        // namespace in hex = 0x000000000000000000000000000000000000736f762d6d696e692d61
        let namespace = "c292LW1pbmktYQ==";
        let commitment = "JkVWHw0eLp6eeCEG28rLwF1xwUWGDI3+DbEyNNKq9fE=";
        let l2_chain_id = 0u64;
        let batch_number = 0u32;

        let blob_id = BlobId::new(
            BlockHeight::from(height),
            Namespace::new_v0(STANDARD.decode(namespace).unwrap().as_slice()).unwrap(),
            Commitment::new(STANDARD.decode(commitment).unwrap().try_into().unwrap()),
        );

        let job_id = JobId {
            blob_id,
            l2_chain_id,
            batch_number,
        };

        let job_id_to_str = job_id.to_string();
        let job_id_from_str = JobId::from_str(
            "6952283:c292LW1pbmktYQ==:JkVWHw0eLp6eeCEG28rLwF1xwUWGDI3+DbEyNNKq9fE=:0:0",
        )
        .unwrap();

        assert_eq!(job_id_from_str, job_id);
        assert_eq!(
            job_id_to_str,
            "6952283:c292LW1pbmktYQ==:JkVWHw0eLp6eeCEG28rLwF1xwUWGDI3+DbEyNNKq9fE=:0:0"
        );
    }

    #[test]
    fn test_job_id_bincode() {
        let height: u32 = 7640999;
        // namespace in hex = 0x000000000000000000000000000000000000736f762d6d696e692d61
        let namespace = "J3fU2WHHWlJt2A==";
        let commitment = "SLzsmvT0rHZtgxS2yHHB7Hr7N6FkPi/UtUOHW0mtIqQ=";
        let l2_chain_id = 0u64;
        let batch_number = 0u32;

        let blob_id = BlobId::new(
            BlockHeight::from(height),
            Namespace::new_v0(STANDARD.decode(namespace).unwrap().as_slice()).unwrap(),
            Commitment::new(STANDARD.decode(commitment).unwrap().try_into().unwrap()),
        );
        let job_id = JobId::new(blob_id, l2_chain_id, batch_number);

        println!("job_id: {:?}", job_id);
        let job_id_bincode = bincode::serialize(&job_id).unwrap();
        let job_id_from_bincode: JobId = bincode::deserialize(&job_id_bincode).unwrap();

        assert_eq!(job_id_from_bincode, job_id);
    }
}
