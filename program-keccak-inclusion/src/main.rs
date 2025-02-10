#![doc = include_str!("../README.md")]
#![no_main]

sp1_zkvm::entrypoint!(main);
use alloy::{primitives::B256, sol_types::SolType};
use celestia_types::{
    blob::Blob,
    nmt::{MerkleHash, NamespacedHashExt},
    AppVersion,
    hash::Hash,
    nmt::NamespaceProof,
    RowProof,
    ShareProof
};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
use nmt_rs::TmSha2Hasher;
use sha3::{Digest, Keccak256};
/*use tendermint::Hash as TmHash;
use tendermint_proto::Protobuf;*/

pub fn main() {
    let input: KeccakInclusionToDataRootProofInput = sp1_zkvm::io::read();
    let data_root_as_hash= Hash::Sha256(input.data_root);

    let blob = Blob::new(input.namespace_id, input.data, AppVersion::V3)
        .expect("Failed creating blob");

    let keccak_hash: [u8; 32] = Keccak256::new()
        .chain_update(&blob.data)
        .finalize()
        .into();

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

    //input.proof.verify(data_root_as_hash).expect("Failed verifying proof");

    //input.

    let output: Vec<u8> = KeccakInclusionToDataRootProofOutput::abi_encode(&(
        B256::from(keccak_hash),
        B256::from(input.data_root),
    ));
    sp1_zkvm::io::commit_slice(&output);
}
