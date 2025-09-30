#![no_main]
#![doc = include_str!("../README.md")]

sp1_zkvm::entrypoint!(main);

use celestia_types::hash::Hash;
use eq_common::{compute_share_raw_data_keccak, ZKStackEqProofInput, ZKStackEqProofOutput};

pub fn main() {
    println!("cycle-tracker-start: deserialize");
    let input: ZKStackEqProofInput = sp1_zkvm::io::read();
    println!("cycle-tracker-end: deserialize");

    println!("cycle-tracker-start: verify NMT proof");
    let wrapped_root_hash = Hash::Sha256(input.data_availability_root);
    input
        .share_proof
        .verify(wrapped_root_hash)
        .expect("NMT proof failed");
    println!("cycle-tracker-end: verify NMT proof");

    println!("cycle-tracker-start: compute keccak hash from shares");
    let computed_keccak = compute_share_raw_data_keccak(
        &input.share_proof.data,
        input.share_version,
        input.tail_padding,
    );
    println!("cycle-tracker-end: compute keccak hash from shares");

    println!("cycle-tracker-start: commit");
    let output = ZKStackEqProofOutput {
        keccak_hash: computed_keccak,
        data_availability_root: input.data_availability_root,
        batch_number: input.batch_number,
        chain_id: input.chain_id,
    }
    .to_vec();
    sp1_zkvm::io::commit_slice(&output);
    println!("cycle-tracker-end: commit");
}
