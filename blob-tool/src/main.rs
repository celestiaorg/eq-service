#![doc = include_str!("../README.md")]

use base64::Engine;
use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::ShareProof;
use celestia_types::blob::Commitment;
use celestia_types::nmt::Namespace;
use clap::{Parser, command};
use eq_common::KeccakInclusionToDataRootProofInput;
use sha3::{Digest, Keccak256};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    height: u64,
    #[arg(long)]
    namespace: String,
    #[arg(long)]
    commitment: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN").expect("Token not provided");
    let client = Client::new("ws://localhost:26658", Some(&node_token))
        .await
        .expect("Failed creating celestia rpc client");

    let header = client
        .header_get_by_height(args.height)
        .await
        .expect("Failed getting header");

    let eds_row_roots = header.dah.row_roots();
    let eds_size: u64 = eds_row_roots.len().try_into().unwrap();
    let ods_size: u64 = eds_size / 2;

    let commitment = Commitment::new(
        base64::engine::general_purpose::STANDARD
            .decode(&args.commitment)
            .expect("Invalid commitment base64")
            .try_into()
            .expect("Invalid commitment length"),
    );

    let namespace =
        Namespace::new_v0(&hex::decode(&args.namespace).expect("Invalid namespace hex"))
            .expect("Invalid namespace");

    println!("getting blob...");
    let blob = client
        .blob_get(args.height, namespace, commitment)
        .await
        .expect("Failed getting blob");

    println!(
        "shares len {:?}, starting index {:?}",
        blob.shares_len(),
        blob.index
    );

    let _index = blob.index.unwrap();
    //let first_row_index: u64 = index.div_ceil(eds_size) - 1;
    // Trying this Diego's way
    let first_row_index: u64 = blob.index.unwrap() / eds_size;
    let ods_index = blob.index.unwrap() - (first_row_index * ods_size);

    let range_response = client
        .share_get_range(&header, ods_index, ods_index + blob.shares_len() as u64)
        .await
        .expect("Failed getting shares");

    range_response
        .proof
        .verify(header.dah.hash())
        .expect("Failed verifying proof");

    let keccak_hash: [u8; 32] = Keccak256::new().chain_update(&blob.data).finalize().into();

    let proof_input = KeccakInclusionToDataRootProofInput {
        data: blob.clone().data,
        namespace_id: namespace,
        share_proofs: range_response.clone().proof.share_proofs,
        row_proof: range_response.clone().proof.row_proof,
        data_root: header.dah.hash().as_bytes().try_into().unwrap(),
        keccak_hash: keccak_hash,
    };

    // create a ShareProof from the KeccakInclusionToDataRootProofInput and verify it
    let share_proof = ShareProof {
        data: blob
            .to_shares()
            .expect("Failed to convert blob to shares")
            .into_iter()
            .map(|share| share.as_ref().try_into().unwrap())
            .collect(),
        namespace_id: namespace,
        share_proofs: proof_input.clone().share_proofs,
        row_proof: proof_input.clone().row_proof,
    };

    share_proof
        .verify(header.dah.hash())
        .expect("Failed verifying proof");

    let json =
        serde_json::to_string_pretty(&proof_input).expect("Failed serializing proof input to JSON");

    std::fs::write("proof_input.json", json).expect("Failed writing proof input to file");

    println!("Wrote proof input to proof_input.json");
}
