use alloy::sol;
use celestia_types::nmt::{Namespace, NamespaceProof};
use celestia_types::RowProof;
use serde::{Deserialize, Serialize};

#[cfg(feature = "utils")]
mod error;
#[cfg(feature = "utils")]
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
    pub data: Vec<u8>,
    pub namespace_id: Namespace,
    pub share_proofs: Vec<NamespaceProof>,
    pub row_proof: RowProof,
    pub data_root: [u8; 32],
    pub keccak_hash: [u8; 32],
}

/// Expecting bytes:
/// (keccak_hash: [u8; 32], pub data_root: [u8; 32])
pub type KeccakInclusionToDataRootProofOutput = sol! {
    tuple(bytes32, bytes32)
};
