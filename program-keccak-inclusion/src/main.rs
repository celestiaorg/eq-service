#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use alloy::{primitives::B256, sol_types::SolType};
use celestia_types::{blob::Blob, hash::Hash, AppVersion, ShareProof};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
use sha3::{Digest, Keccak256};

pub fn main() {
    let input: KeccakInclusionToDataRootProofInput = sp1_zkvm::io::read();
    let data_root_as_hash = Hash::Sha256(input.data_root);

    let blob =
        Blob::new(input.namespace_id, input.data, AppVersion::V3).expect("Failed creating blob");

    let computed_keccak_hash: [u8; 32] =
        Keccak256::new().chain_update(&blob.data).finalize().into();

    let rp = ShareProof {
        data: blob
            .to_shares()
            .expect("Failed to convert blob to shares")
            .into_iter()
            .map(|share| share.as_ref().try_into().unwrap())
            .collect(),
        namespace_id: input.namespace_id,
        share_proofs: input.share_proofs,
        row_proof: input.row_proof,
    };

    println!("Verifying proof");
    rp.verify(data_root_as_hash)
        .expect("Failed verifying proof");

    println!("Checking keccak hash");
    if computed_keccak_hash != input.keccak_hash {
        panic!("Computed keccak hash does not match input keccak hash");
    }

    let output: Vec<u8> = KeccakInclusionToDataRootProofOutput::abi_encode(&(
        B256::from(computed_keccak_hash),
        B256::from(input.data_root),
    ));
    sp1_zkvm::io::commit_slice(&output);
}
