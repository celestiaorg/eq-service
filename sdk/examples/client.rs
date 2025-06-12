use clap::Parser;
use eq_sdk::{types::BlobId, EqClient};
use tonic::transport::Endpoint;

#[derive(Parser, Debug)]
#[command(author, version)]
#[command(disable_help_flag(true))]
struct Args {
    /// RPC endpoint (e.g. "127.0.0.1:50051" or "http://…")
    #[arg(short, long, env = "EQ_SOCKET")]
    socket: String,

    /// Block height (u64)
    #[arg(short = 'h', long)]
    height: u64,

    /// Namespace (base64-encoded)
    #[arg(short, long)]
    namespace: String,

    /// Commitment (base64-encoded, 32 bytes)
    #[arg(short, long)]
    commitment: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Build a valid URL for tonic:
    let url = if args.socket.starts_with("http") {
        args.socket.clone()
    } else {
        format!("http://{}", args.socket)
    };

    // Connect
    let channel = Endpoint::from_shared(url)?
        .connect()
        .await
        .map_err(|e| format!("gRPC connect error: {e}"))?;
    let client = EqClient::new(channel);

    // Reconstruct the canonical "height:namespace:commitment" string
    let blob_str = format!("{}:{}:{}", args.height, args.namespace, args.commitment);

    // And hand it off to your existing BlobId::from_str impl:
    let blob_id: BlobId = blob_str.parse()?;

    // Call the RPC
    let resp = client.get_keccak_inclusion(&blob_id).await?;
    println!("{:#?}", resp);

    Ok(())
}
