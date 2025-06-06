use celestia_types::{
    nmt::{Namespace, NamespaceProof},
    RowProof,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "host")]
mod error;
#[cfg(feature = "host")]
pub use error::InclusionServiceError;

#[cfg(feature = "grpc")]
/// gRPC generated bindings
pub mod eqs {
    include!("generated/eqs.rs");
}

/*
    The types of proofs we expect to support:
    1. KeccakInclusionToDataRootProof
    2. KeccakInclusionToBlockHashProof
    3. PayyPoseidonToDataRootProof
    4. PayyPoseidonToBlockHashProof
*/

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct KeccakInclusionToDataRootProofInput {
    #[serde(rename = "blob_data")]
    pub data: Vec<u8>,

    #[serde(rename = "blob_namespace")]
    pub namespace_id: Namespace,

    #[serde(rename = "nmt_multiproofs")]
    pub share_proofs: Vec<NamespaceProof>,

    #[serde(rename = "row_root_multiproof")]
    pub row_proof: RowProof,

    pub data_root: [u8; 32],   // already matches
    pub keccak_hash: [u8; 32], // already matches
}

/// Expecting bytes:
/// (keccak_hash: [u8; 32], pub data_root: [u8; 32])
pub struct KeccakInclusionToDataRootProofOutput {
    pub keccak_hash: [u8; 32],
    pub data_root: [u8; 32],
}
impl KeccakInclusionToDataRootProofOutput {
    // Simple encoding, rather than use any Ethereum libraries
    pub fn to_vec(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.keccak_hash);
        encoded.extend_from_slice(&self.data_root);
        encoded
    }

    #[cfg(feature = "host")]
    pub fn from_bytes(data: &[u8]) -> Result<Self, InclusionServiceError> {
        if data.len() != 64 {
            return Err(InclusionServiceError::OutputDeserializationError);
        }
        let decoded = KeccakInclusionToDataRootProofOutput {
            keccak_hash: data[0..32]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
            data_root: data[32..64]
                .try_into()
                .map_err(|_| InclusionServiceError::OutputDeserializationError)?,
        };
        Ok(decoded)
    }
}

pub struct PayyInclusionToDataRootProofInput {
    pub data: Vec<u8>,
    pub namespace_id: Namespace,
    pub share_proofs: Vec<NamespaceProof>,
    pub row_proof: RowProof,
    pub data_root: [u8; 32],
    // the ~8 UTXO hashes for a Payy block, bn254 scalar field elements represented as [u8; 32].
    pub hashes: Vec<[u8; 32]>,
}

pub struct PayyInclusionToDataRootProofOutput {
    pub data_root: [u8; 32],
    // The Payy hash of the leaves, a bn254 scalar field element represented as [u8; 32].
    pub payy_commitment: [u8; 32],
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg(feature = "host")]
    fn test_abi_encoding() {
        let output = KeccakInclusionToDataRootProofOutput {
            keccak_hash: [0; 32],
            data_root: [0; 32],
        };
        let encoded = output.to_vec();
        let decoded = KeccakInclusionToDataRootProofOutput::from_bytes(&encoded).unwrap();
        assert_eq!(output.keccak_hash, decoded.keccak_hash);
        assert_eq!(output.data_root, decoded.data_root);
    }
}
