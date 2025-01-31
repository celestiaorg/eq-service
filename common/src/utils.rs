use crate::*;

use celestia_types::{blob::Blob, nmt::NamespacedHashExt, ExtendedHeader};
use nmt_rs::{
    simple_merkle::{
        db::MemDb,
        tree::{MerkleHash, MerkleTree},
    },
    TmSha2Hasher,
};
use sha3::{Digest, Keccak256};
use std::cmp::max;
use tendermint_proto::Protobuf;

pub fn create_inclusion_proof_input(
    blob: &Blob,
    header: &ExtendedHeader,
    nmt_multiproofs: Vec<NamespaceProof>,
) -> Result<KeccakInclusionToDataRootProofInput, InclusionServiceError> {
    let eds_row_roots = header.dah.row_roots();
    let eds_column_roots = header.dah.column_roots();

    // Compute these values needed for proving inclusion
    let eds_size: u64 = eds_row_roots.len().try_into().unwrap();
    let ods_size = eds_size / 2;

    let blob_index = blob.index.ok_or(InclusionServiceError::MissingBlobIndex)?;
    let blob_size: u64 = max(
        1,
        blob.to_shares()
            .map_err(|e| InclusionServiceError::ShareConversionError(e.to_string()))?
            .len() as u64,
    );
    let first_row_index: u64 = blob_index.div_ceil(eds_size) - 1;
    let ods_index = blob_index - (first_row_index * ods_size);

    let last_row_index: u64 = (ods_index + blob_size).div_ceil(ods_size) - 1;

    let hasher = TmSha2Hasher {};
    let mut row_root_tree: MerkleTree<MemDb<[u8; 32]>, TmSha2Hasher> =
        MerkleTree::with_hasher(hasher);

    let leaves = eds_row_roots
        .iter()
        .chain(eds_column_roots.iter())
        .map(|root| root.to_array())
        .collect::<Vec<[u8; 90]>>();

    for root in &leaves {
        row_root_tree.push_raw_leaf(root);
    }

    // assert that the row root tree equals the data hash
    assert_eq!(
        row_root_tree.root(),
        header.header.data_hash.unwrap().as_bytes()
    );
    // Get range proof of the row roots spanned by the blob
    // +1 is so we include the last row root
    let row_root_multiproof =
        row_root_tree.build_range_proof(first_row_index as usize..(last_row_index + 1) as usize);
    // Sanity check, verify the row root range proof
    let hasher = TmSha2Hasher {};
    let leaves_hashed = leaves
        .iter()
        .map(|leaf| hasher.hash_leaf(leaf))
        .collect::<Vec<[u8; 32]>>();
    row_root_multiproof
        .verify_range(
            header
                .header
                .data_hash
                .unwrap()
                .as_bytes()
                .try_into()
                .unwrap(),
            &leaves_hashed[first_row_index as usize..(last_row_index + 1) as usize],
        )
        .map_err(|_| InclusionServiceError::RowRootVerificationFailed)?;

    let mut hasher = Keccak256::new();
    hasher.update(&blob.data);
    let hash: [u8; 32] = hasher
        .finalize()
        .try_into()
        .map_err(|_| InclusionServiceError::KeccakHashConversion)?;

    Ok(KeccakInclusionToDataRootProofInput {
        blob_data: blob.data.clone(),
        blob_index: blob.index.unwrap(),
        blob_namespace: blob.namespace,
        keccak_hash: hash,
        nmt_multiproofs,
        row_root_multiproof,
        row_roots: eds_row_roots[first_row_index as usize..=last_row_index as usize].to_vec(),
        data_root: header.header.data_hash.unwrap().encode_vec(),
    })
}
