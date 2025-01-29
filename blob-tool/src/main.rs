#![doc = include_str!("../README.md")]

use base64::Engine;
use celestia_rpc::{BlobClient, Client, HeaderClient};
use celestia_types::blob::Commitment;
use celestia_types::nmt::Namespace;
use clap::{command, Parser};
use eq_common::create_inclusion_proof_input;

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

    println!("getting nmt multiproofs...");
    let nmt_multiproofs = client
        .blob_get_proof(args.height, namespace, commitment)
        .await
        .expect("Failed getting nmt multiproofs");

    let input = create_inclusion_proof_input(&blob, &header, nmt_multiproofs)
        .expect("Failed creating inclusion proof input");

    let json =
        serde_json::to_string_pretty(&input).expect("Failed serializing proof input to JSON");

    std::fs::write("proof_input.json", json).expect("Failed writing proof input to file");

    println!("Wrote proof input to proof_input.json");
}
