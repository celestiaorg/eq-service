#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use celestia_types::{hash::Hash, ShareProof};
use eq_common::{ZKStackEqProofInput, ZKStackEqProofOutput};
use sha3::{Digest, Keccak256};

pub fn main() {
    println!("cycle-tracker-start: deserialize input");
    let input: ZKStackEqProofInput = sp1_zkvm::io::read();
    let data_root_as_hash = Hash::Sha256(input.data_root);
    println!("cycle-tracker-end: deserialize input");

    println!("cycle-tracker-start: compute keccak hash");
    let computed_keccak_hash: [u8; 32] = Keccak256::digest(&input.blob_data).into();
    println!("cycle-tracker-end: compute keccak hash");

    println!("cycle-tracker-start: convert blob to shares");
    let rp = ShareProof {
        data: input.shares_data.into_iter().map(|s| s.into()).collect(),
        namespace_id: input.blob_namespace,
        share_proofs: input.nmt_multiproofs,
        row_proof: input.row_root_multiproof,
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
    let output: Vec<u8> = ZKStackEqProofOutput {
        keccak_hash: computed_keccak_hash,
        data_root: input.data_root,
        batch_number: input.batch_number,
        chain_id: input.chain_id,
    }
    .to_vec();
    sp1_zkvm::io::commit_slice(&output);
    println!("cycle-tracker-end: commit output");
}
