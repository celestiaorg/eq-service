#![doc = include_str!("../README.md")]

use base64::Engine;
use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::blob::Commitment;
use celestia_types::nmt::Namespace;
use clap::{command, Parser};
use eq_common::{KeccakInclusionToDataRootProofInput, KeccakInclusionToDataRootProofOutput};
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

    println!("shares len {:?}, starting index {:?}", blob.shares_len(), blob.index);

    let index = blob.index.unwrap();
    let range_response = client
        .share_get_range(&header, index, index + blob.shares_len() as u64)
        .await
        .expect("Failed getting shares");

    range_response.proof.verify(header.dah.hash())
        .expect("Failed verifying proof");

    let keccak_hash: [u8; 32] = Keccak256::new()
        .chain_update(&blob.data)
        .finalize()
        .into();

    let proof_input = KeccakInclusionToDataRootProofInput {
        data: blob.data,
        namespace_id: namespace,
        share_proofs: range_response.proof.share_proofs,
        row_proof: range_response.proof.row_proof,
        data_root: header.dah.hash().as_bytes().try_into().unwrap(),
        keccak_hash: keccak_hash,
    };

    let json = serde_json::to_string_pretty(&proof_input).expect("Failed serializing proof input to JSON");

    std::fs::write("proof_input.json", json).expect("Failed writing proof input to file");

    println!("Wrote proof input to proof_input.json");
}
