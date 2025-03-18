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

#[cfg(test)]
mod test {
    use super::*;

    use alloy_primitives::FixedBytes;
    use alloy_sol_types::{SolType, SolValue};

    #[test]
    fn test_abi_encoding() {
        let f: FixedBytes<32> = FixedBytes::from([0; 32]);
        let output = (
            FixedBytes::<32>::from([0; 32]),
            FixedBytes::<32>::from([0; 32]),
        );
        let encoded = output.abi_encode();
        // Interestingly, this line doesn't work
        // You can't get a KeccakInclusionToDataRootProofOutput by calling KeccakInclusionToDataRootProofOutput::abi_decode
        // However, we can get a tuple of alloy_primitives::FixedBytes<32>
        // which is weird but fine
        /*let decoded: KeccakInclusionToDataRootProofOutput = KeccakInclusionToDataRootProofOutput::abi_decode(&encoded, false)
        .unwrap();*/
        let decoded: (FixedBytes<32>, FixedBytes<32>) =
            KeccakInclusionToDataRootProofOutput::abi_decode(&encoded, true).unwrap();
        assert_eq!(output, decoded);
    }
}
