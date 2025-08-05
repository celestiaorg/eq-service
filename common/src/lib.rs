use celestia_types::{
    nmt::{Namespace, NamespaceProof},
    RowProof,
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

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
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZKStackEqProofInput {
    /// Celestia App version
    pub app_version: u64,
    pub blob_data: Vec<u8>,
    pub blob_namespace: Namespace,
    pub nmt_multiproofs: Vec<NamespaceProof>,
    pub row_root_multiproof: RowProof,
    #[serde_as(as = "Bytes")]
    pub data_root: [u8; 32],
    #[serde_as(as = "Bytes")]
    pub keccak_hash: [u8; 32],
    // batch_number and chain_id are passed through to prevent proofs from being replayed
    pub batch_number: u32,
    pub chain_id: u64,
}

/// Expecting bytes:
/// (keccak_hash: [u8; 32], pub data_root: [u8; 32])
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
