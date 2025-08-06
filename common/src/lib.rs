use celestia_types::{
    consts::appconsts::{
        CONTINUATION_SPARSE_SHARE_CONTENT_SIZE, FIRST_SPARSE_SHARE_CONTENT_SIZE, NAMESPACE_SIZE,
        SEQUENCE_LEN_BYTES, SHARE_INFO_BYTES, SHARE_SIZE, SIGNER_SIZE,
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

/// Computes Keccak‐256 over the reconstructed blob bytes by streaming
/// payload portions of each share, skipping headers and, for version 1, the signer.
/// Supports share versions 0 and 1; panics on unknown versions.
///
/// See: https://celestiaorg.github.io/celestia-app/shares.html#share-version
pub fn compute_blob_keccak(raw_shares: Vec<[u8; SHARE_SIZE]>) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    let mut iter = raw_shares.iter();

    if let Some(first_share) = iter.next() {
        let bytes = first_share.as_ref();
        let info_offset = NAMESPACE_SIZE;
        let info = bytes[info_offset];
        let version = info >> 1;

        let mut offset = info_offset + SHARE_INFO_BYTES + SEQUENCE_LEN_BYTES;
        if version == 1 {
            offset += SIGNER_SIZE;
        }

        let content_len = FIRST_SPARSE_SHARE_CONTENT_SIZE;

        match version {
            0 | 1 => hasher.update(&bytes[offset..offset + content_len]),
            other => panic!("unsupported share version {} in first share", other),
        }
    }

    for share in iter {
        let bytes = share.as_ref();
        let info = bytes[NAMESPACE_SIZE];
        let version = info >> 1; // same logic for share version
        let offset = NAMESPACE_SIZE + SHARE_INFO_BYTES;
        let content_len = CONTINUATION_SPARSE_SHARE_CONTENT_SIZE;

        match version {
            0 | 1 => hasher.update(&bytes[offset..offset + content_len]),
            other => panic!("unsupported share version {} in continuation share", other),
        };
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
