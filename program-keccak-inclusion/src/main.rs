#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use celestia_types::{blob::Blob, hash::Hash, AppVersion, ShareProof};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
use sha3::{Digest, Keccak256};

pub fn main() {
    println!("cycle-tracker-start: deserialize input");
    let input: KeccakInclusionToDataRootProofInput = sp1_zkvm::io::read();
    let data_root_as_hash = Hash::Sha256(input.data_root);
    println!("cycle-tracker-end: deserialize input");

    println!("cycle-tracker-start: create blob");
    let blob =
        Blob::new(input.namespace_id, input.data, AppVersion::V3).expect("Failed creating blob");
    println!("cycle-tracker-end: create blob");

    println!("cycle-tracker-start: compute keccak hash");
    let computed_keccak_hash: [u8; 32] =
        Keccak256::new().chain_update(&blob.data).finalize().into();
    println!("cycle-tracker-end: compute keccak hash");

    println!("cycle-tracker-start: convert blob to shares");
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
    println!("cycle-tracker-end: convert blob to shares");

    println!("cycle-tracker-start: verify proof");
    rp.verify(data_root_as_hash)
        .expect("Failed verifying proof");
    println!("cycle-tracker-end: verify proof");

    println!("cycle-tracker-start: check keccak hash");
    if computed_keccak_hash != input.keccak_hash {
        panic!("Computed keccak hash does not match input keccak hash");
    }
    println!("cycle-tracker-end: check keccak hash");

    println!("cycle-tracker-start: commit output");
    let output: Vec<u8> = KeccakInclusionToDataRootProofOutput {
        keccak_hash: computed_keccak_hash,
        data_root: input.data_root,
    }
    .to_vec();
    sp1_zkvm::io::commit_slice(&output);
    println!("cycle-tracker-end: commit output");
}
