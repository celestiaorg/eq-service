use celestia_types::{
    consts::appconsts::{
        CONTINUATION_SPARSE_SHARE_CONTENT_SIZE, FIRST_SPARSE_SHARE_CONTENT_SIZE, NAMESPACE_SIZE,
        SEQUENCE_LEN_BYTES, SHARE_INFO_BYTES, SHARE_SIZE,
    },
    ShareProof,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

#[cfg(feature = "host")]
mod error;
#[cfg(feature = "host")]
pub use error::{ErrorLabels, InclusionServiceError};

#[cfg(feature = "grpc")]
/// gRPC generated bindings
pub mod eqs {
    include!("generated/eqs.rs");
}

/*
    For now, we only support ZKStackEqProofs
    These are used for Celestia integrations with Matter Labs' ZKStack
    TODO: Add support for Payy Celestia integration
*/
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZKStackEqProofInput {
    pub share_proof: ShareProof,
    pub data_root: [u8; 32],
    pub batch_number: u32,
    pub chain_id: u64,
}

pub struct ZKStackEqProofOutput {
    pub keccak_hash: [u8; 32],
    pub data_root: [u8; 32],
    pub batch_number: u32,
    pub chain_id: u64,
}

impl ZKStackEqProofOutput {
    // Simple encoding, rather than use any Ethereum libraries
    pub fn to_vec(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.keccak_hash);
        encoded.extend_from_slice(&self.data_root);
        encoded.extend_from_slice(&self.batch_number.to_le_bytes());
        encoded.extend_from_slice(&self.chain_id.to_le_bytes());
        encoded
    }

    #[cfg(feature = "host")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, InclusionServiceError> {
        if data.len() != 76 {
            return Err(InclusionServiceError::OutputDeserializationError);
        }
        let decoded = ZKStackEqProofOutput {
            keccak_hash: data[0..32]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            data_root: data[32..64]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            batch_number: u32::from_le_bytes(
                data[64..68]
                    .try_into()
                    .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            ),
            chain_id: u64::from_le_bytes(
                data[68..76]
                    .try_into()
                    .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            ),
        };
        Ok(decoded)
    }
}

/// Computes Keccak-256 over the reconstructed blob bytes by streaming
/// payload portions of each share, skipping headers.
/// See https://docs.rs/celestia-types/latest/celestia_types/struct.Share.html
pub fn compute_blob_keccak(raw_shares: Vec<[u8; SHARE_SIZE]>) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for share in raw_shares.iter() {
        let bytes = share.as_ref();
        // skip namespace ID + info byte
        let info_offset = NAMESPACE_SIZE;
        let info = bytes[info_offset];
        let is_start = (info & 0b0000_0001) != 0;
        // calculate payload start
        let mut offset = info_offset + SHARE_INFO_BYTES;
        if is_start {
            offset += SEQUENCE_LEN_BYTES;
        }
        // determine actual content length (exclude padding)
        let content_len = if is_start {
            FIRST_SPARSE_SHARE_CONTENT_SIZE
        } else {
            CONTINUATION_SPARSE_SHARE_CONTENT_SIZE
        };
        // absorb only the actual data bytes
        hasher.update(&bytes[offset..offset + content_len]);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg(feature = "host")]
    fn test_serialization() {
        let output = ZKStackEqProofOutput {
            keccak_hash: [0; 32],
            data_root: [0; 32],
            batch_number: 0u32,
            chain_id: 0u64,
        };
        let encoded = output.to_vec();
        let decoded = ZKStackEqProofOutput::from_bytes(&encoded).unwrap();
        assert_eq!(output.keccak_hash, decoded.keccak_hash);
        assert_eq!(output.data_root, decoded.data_root);
    }
}
