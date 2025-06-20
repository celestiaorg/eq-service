#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use celestia_types::{blob::Blob, hash::Hash, AppVersion, ShareProof};
use eq_common::{PayyInclusionToDataRootProofInput, PayyInclusionToDataRootProofOutput};
use sha3::{Digest, Keccak256};
use sp1_bn254_poseidon::fields::bn256::FpBN256;

pub fn main() {
    println!("cycle-tracker-start: deserialize input");
    let input: PayyInclusionToDataRootProofInput = sp1_zkvm::io::read();
    println!("cycle-tracker-end: deserialize input");

    println!("cycle-tracker-start: create blob");
    let blob =
        Blob::new(input.namespace_id, input.data, AppVersion::V3).expect("Failed creating blob");
    println!("cycle-tracker-end: create blob");

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

    let computed_payy_commitment: [u8; 32] = compute_payy_commitment(&input.hashes);

    println!("cycle-tracker-start: commit output");
    let output: Vec<u8> = PayyInclusionToDataRootProofOutput {
        payy_commitment: computed_payy_commitment,
        data_root: input.data_root,
    }
    .to_vec();
    sp1_zkvm::io::commit_slice(&output);
    println!("cycle-tracker-end: commit output");
}
