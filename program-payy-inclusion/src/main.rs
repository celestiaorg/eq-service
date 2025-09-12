#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use celestia_types::{blob::Blob, hash::Hash, AppVersion, ShareProof};
use eq_common::{PayyInclusionToDataRootProofInput, PayyInclusionToDataRootProofOutput};
use sha3::{Digest, Keccak256};
use sp1_bn254_poseidon::fields::bn256::FpBN256;

pub fn main() {

    println!("payy-inclusion-start: deserialize input");
    let input: PayyInclusionToDataRootProofInput = sp1_zkvm::io::read();
    println!("payy-inclusion-end: deserialize input");

    println!("payy-inclusion-start: create blob");
    let blob = Blob::new(input.namespace_id, input.data, AppVersion::V3).expect("Failed creating blob");
    println!("payy-inclusion-end: create blob");

    println!("payy-inclusion-start: convert blob to shares");
}
