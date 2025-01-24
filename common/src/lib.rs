use celestia_types::{
    blob::Blob,
    nmt::{Namespace, NamespaceProof, NamespacedHashExt},
    ExtendedHeader,
};
use nmt_rs::{
    simple_merkle::{
        db::MemDb,
        proof::Proof,
        tree::{MerkleHash, MerkleTree},
    },
    NamespacedHash, TmSha2Hasher,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::cmp::max;
use tendermint::hash::Hash as TmHash;
use tendermint_proto::{
    v0_37::{types::BlockId as RawBlockId, version::Consensus as RawConsensusVersion},
    Protobuf,
};

mod error;
pub use error::InclusionServiceError;

/*
    The types of proofs we expect to support:
    1. KeccakInclusionToDataRootProof
    2. KeccakInclusionToBlockHashProof
    3. PayyPoseidonToDataRootProof
    4. PayyPoseidonToBlockHashProof
*/

#[derive(Serialize, Deserialize)]
pub struct KeccakInclusionToDataRootProofInput {
    pub blob_data: Vec<u8>,
    pub blob_index: u64,
    pub blob_namespace: Namespace,
    pub nmt_multiproofs: Vec<NamespaceProof>,
    pub row_root_multiproof: Proof<TmSha2Hasher>,
    pub row_roots: Vec<NamespacedHash<29>>,
    pub data_root: Vec<u8>,
    pub keccak_hash: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct KeccakInclusionToDataRootProofOutput {
    pub keccak_hash: [u8; 32],
    pub data_root: Vec<u8>,
}

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

pub fn create_header_field_tree(
    header: &ExtendedHeader,
) -> (
    MerkleTree<MemDb<[u8; 32]>, TmSha2Hasher>,
    Proof<TmSha2Hasher>,
) {
    let hasher = TmSha2Hasher {};
    let mut header_field_tree: MerkleTree<MemDb<[u8; 32]>, TmSha2Hasher> =
        MerkleTree::with_hasher(hasher);

    let field_bytes = vec![
        Protobuf::<RawConsensusVersion>::encode_vec(header.header.version),
        header.header.chain_id.clone().encode_vec(),
        header.header.height.encode_vec(),
        header.header.time.encode_vec(),
        Protobuf::<RawBlockId>::encode_vec(header.header.last_block_id.unwrap_or_default()),
        header
            .header
            .last_commit_hash
            .unwrap_or_default()
            .encode_vec(),
        header.header.data_hash.unwrap_or_default().encode_vec(),
        header.header.validators_hash.encode_vec(),
        header.header.next_validators_hash.encode_vec(),
        header.header.consensus_hash.encode_vec(),
        header.header.app_hash.clone().encode_vec(),
        header
            .header
            .last_results_hash
            .unwrap_or_default()
            .encode_vec(),
        header.header.evidence_hash.unwrap_or_default().encode_vec(),
        header.header.proposer_address.encode_vec(),
    ];

    for leaf in field_bytes {
        header_field_tree.push_raw_leaf(&leaf);
    }

    let (data_hash_bytes_from_tree, data_hash_proof) = header_field_tree.get_index_with_proof(6);

    // Verify the data hash
    let data_hash_from_tree = TmHash::decode_vec(&data_hash_bytes_from_tree).unwrap();
    assert_eq!(
        data_hash_from_tree.as_bytes(),
        header.header.data_hash.unwrap().as_bytes()
    );
    assert_eq!(header.hash().as_ref(), header_field_tree.root());

    // Verify the proof
    let hasher = TmSha2Hasher {};
    data_hash_proof
        .verify_range(
            &header_field_tree.root(),
            &[hasher.hash_leaf(&data_hash_bytes_from_tree)],
        )
        .unwrap();

    (header_field_tree, data_hash_proof)
}
