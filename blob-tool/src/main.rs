#![doc = include_str!("../README.md")]

use base64::Engine;
use celestia_rpc::{BlobClient, Client, HeaderClient, ShareClient};
use celestia_types::{blob::Commitment, nmt::Namespace, ShareProof};
use clap::Parser;
use eq_common::ZKStackEqProofInput;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    height: u64,
    #[arg(long)]
    namespace: String,
    #[arg(long)]
    commitment: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let node_token = std::env::var("CELESTIA_NODE_AUTH_TOKEN")?;
    let client = Client::new("http://127.0.0.1:26658", Some(&node_token)).await?;

    let header = client.header_get_by_height(args.height).await?;

    let ns_bytes = hex::decode(&args.namespace)?;
    let namespace = Namespace::new_v0(&ns_bytes)?;

    let comm_bytes = base64::engine::general_purpose::STANDARD.decode(&args.commitment)?;
    let commitment = Commitment::new(comm_bytes.try_into().expect("commitment construction"));

    let blob = client.blob_get(args.height, namespace, commitment).await?;

    // Compute sizes & indices
    let eds_size = header.dah.row_roots().len() as u64;
    let ods_size = eds_size / 2;
    let idx = blob.index.ok_or("Blob index missing").expect("blob index");
    let first_row = idx / eds_size;
    let ods_index = idx - first_row * ods_size;

    // Fetch & verify the inclusion proof
    let range = client
        .share_get_range(&header, ods_index, ods_index + blob.shares_len() as u64)
        .await?;
    range.proof.verify(header.dah.hash())?;

    // Build the ShareProof
    let share_proof = ShareProof {
        data: blob
            .to_shares()?
            .into_iter()
            .map(|s| s.as_ref().try_into().unwrap())
            .collect(),
        namespace_id: namespace,
        share_proofs: range.proof.share_proofs.clone(),
        row_proof: range.proof.row_proof.clone(),
    };

    // Do a sanity check
    share_proof.verify(header.dah.hash())?;

    let proof_input = ZKStackEqProofInput {
        share_proof,
        data_root: header.dah.hash().as_bytes().try_into()?,
        batch_number: 0,
        chain_id: 0,
    };

    let json = serde_json::to_string_pretty(&proof_input)?;
    std::fs::write("proof_input.json", json)?;

    println!("Wrote proof_input.json");
    Ok(())
}
