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
    pub share_version: bool,
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

/// Computes Keccak‐256 over the reconstructed blob bytes from shares.
/// Supports share versions 0 (false) and 1 (true).
///
/// See: https://celestiaorg.github.io/celestia-app/shares.html#share-version
///
/// The caller MUST:
/// - Pass a non-empty, well-formed sequence of shares for a single blob
/// - Ensure `first_version` is the correct share version for this sequence
/// - Ensure all offsets fit within `SHARE_SIZE`
/// If not, this function may panic.
pub fn compute_blob_keccak(raw_shares: &[[u8; SHARE_SIZE]], share_version: bool) -> [u8; 32] {
    let mut hasher = Keccak256::new();

    // first share (assumed sequence_start = 1, version = first_version)
    let first = raw_shares.first().expect("empty shares");
    update_first_share(&mut hasher, share_version, first.as_ref());

    // continuation shares (assumed sequence_start = 0)
    for share in &raw_shares[1..] {
        update_cont_share(&mut hasher, share.as_ref());
    }

    hasher.finalize().into()
}

/// Fast helpers with no validation. They panic on bad input.
#[inline(always)]
fn update_first_share(hasher: &mut Keccak256, version: bool, bytes: &[u8]) {
    // universal prefix + seq-len (+ signer for v1)
    let base = NAMESPACE_SIZE + SHARE_INFO_BYTES + SEQUENCE_LEN_BYTES;
    let offset = if version { base + SIGNER_SIZE } else { base };
    let end = offset + FIRST_SPARSE_SHARE_CONTENT_SIZE;
    // Intentionally rely on slice bounds (will panic if caller miscomputed).
    hasher.update(&bytes[offset..end]);
}

/// Fast helpers with no validation. They panic on bad input.
#[inline(always)]
fn update_cont_share(hasher: &mut Keccak256, bytes: &[u8]) {
    let offset = NAMESPACE_SIZE + SHARE_INFO_BYTES;
    let end = offset + CONTINUATION_SPARSE_SHARE_CONTENT_SIZE;
    hasher.update(&bytes[offset..end]);
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
