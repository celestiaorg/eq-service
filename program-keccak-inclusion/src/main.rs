#![no_main]
sp1_zkvm::entrypoint!(main);
use celestia_types::{
    blob::Blob,
    nmt::{MerkleHash, NamespacedHashExt},
    AppVersion,
};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
use nmt_rs::TmSha2Hasher;
use sha3::{Digest, Keccak256};
use tendermint::Hash as TmHash;
use tendermint_proto::Protobuf;

pub fn main() {
    println!("cycle-tracker-start: deserializing inputs");
    let input: KeccakInclusionToDataRootProofInput = sp1_zkvm::io::read();
    let data_root = TmHash::decode_vec(&input.data_root).unwrap();
    let mut blob: Blob = Blob::new(input.blob_namespace, input.blob_data, AppVersion::V3)
        .expect("Failed to create blob");
    blob.index = Some(input.blob_index);
    println!("cycle-tracker-end: deserializing inputs");

    println!("cycle-tracker-start: converting blob to shares");
    let shares = blob.to_shares().expect("Failed to convert blob to shares");
    println!("cycle-tracker-end: converting blob to shares");

    println!("cycle-tracker-start: verifying NMT multiproofs");
    let mut start = 0;
    for i in 0..input.nmt_multiproofs.len() {
        let proof = &input.nmt_multiproofs[i];
        let end = start + (proof.end_idx() as usize - proof.start_idx() as usize);
        proof
            .verify_range(
                &input.row_roots[i],
                &shares[start..end],
                blob.namespace.into(),
            )
            .expect("NMT multiproof into row root failed verification"); // Panicking should prevent an invalid proof from being generated
        start = end;
    }
    println!("cycle-tracker-end: verifying NMT multiproofs");

    println!("cycle-tracker-start: verify row root inclusion multiproof");
    let tm_hasher = TmSha2Hasher {};
    let blob_row_root_hashes: Vec<[u8; 32]> = input
        .row_roots
        .iter()
        .map(|root| tm_hasher.hash_leaf(&root.to_array()))
        .collect();
    input
        .row_root_multiproof
        .verify_range(
            data_root
                .as_bytes()
                .try_into()
                .expect("Failed to convert data root to bytes"),
            &blob_row_root_hashes,
        )
        .expect("Row root inclusion multiproof failed verification");
    println!("cycle-tracker-end: verify row root inclusion multiproof");

    println!("cycle-tracker-start: verifying keccak hash inclusion");
    let mut hasher = Keccak256::new();
    hasher.update(&blob.data);
    let hash: [u8; 32] = hasher
        .finalize()
        .try_into()
        .expect("Failed to convert keccak hash to array");
    assert_eq!(
        hash, input.keccak_hash,
        "Keccak hash inclusion failed verification"
    );
    println!("cycle-tracker-end: verifying keccak hash inclusion");

    sp1_zkvm::io::commit(&KeccakInclusionToDataRootProofOutput {
        keccak_hash: hash,
        data_root: data_root.as_bytes().to_vec(),
    });
}
