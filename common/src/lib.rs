use alloy::sol;
use celestia_types::nmt::{Namespace, NamespaceProof};
use nmt_rs::{simple_merkle::proof::Proof, NamespacedHash, TmSha2Hasher};
use serde::{Deserialize, Serialize};

#[cfg(feature = "utils")]
mod error;
#[cfg(feature = "utils")]
pub use error::InclusionServiceError;

#[cfg(feature = "utils")]
pub mod utils;
#[cfg(feature = "utils")]
pub use utils::*;

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

#[derive(Serialize, Deserialize)]
pub struct KeccakInclusionToDataRootProofInput {
    pub blob_data: Vec<u8>,
    pub blob_index: u64,
    pub blob_namespace: Namespace,
    pub nmt_multiproofs: Vec<NamespaceProof>,
    pub row_root_multiproof: Proof<TmSha2Hasher>,
    pub row_roots: Vec<NamespacedHash<29>>,
    pub data_root: Vec<u8>,
    pub keccak_hash: [u8; 32],
}

/// Expecting bytes:
/// (keccak_hash: [u8; 32], pub data_root: [u8; 32])
pub type KeccakInclusionToDataRootProofOutput = sol! {
    tuple(bytes32, bytes32)
};
