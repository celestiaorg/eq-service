#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use alloy::{primitives::B256, sol_types::SolType};
use celestia_types::{
    blob::Blob,
    nmt::{MerkleHash, NamespacedHashExt},
    AppVersion,
    hash::Hash
};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
use nmt_rs::TmSha2Hasher;
use sha3::{Digest, Keccak256};
/*use tendermint::Hash as TmHash;
use tendermint_proto::Protobuf;*/

pub fn main() {
    let input: KeccakInclusionToDataRootProofInput = sp1_zkvm::io::read();
    let data_root: [u8; 32] = sp1_zkvm::io::read();
    let data_root_as_hash= Hash::Sha256(data_root);

    input.proof.verify(data_root_as_hash).expect("Failed verifying proof");

    let output: Vec<u8> = KeccakInclusionToDataRootProofOutput::abi_encode(&(
        B256::from(hash),
        B256::from(data_root_bytes),
    ));
    sp1_zkvm::io::commit_slice(&output);
}
