use eq_sdk::{types::BlobId, EqClient};
use tonic::transport::Endpoint;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service_socket =
        "http://".to_string() + &std::env::var("EQ_SOCKET").expect("EQ_SOCKET env var required");
    let channel = Endpoint::from_shared(service_socket)
        .map_err(|e| format!("gRPC error: {e}"))?
        .connect()
        .await
        .map_err(|e| format!("gRPC error: {e}"))?;
    let client = EqClient::new(channel);

    // "height": 4409088, "namespace": "XSUTEfJbE6VJ4A==", "commitment":"DYoAZpU7FrviV7Ui/AjQv0BpxCwexPWaOW/hQVpEl/s="
    let blob_id: BlobId = "4409088:XSUTEfJbE6VJ4A==:DYoAZpU7FrviV7Ui/AjQv0BpxCwexPWaOW/hQVpEl/s="
        .parse::<BlobId>()
        .map_err(|e| format!("Input error: {e}"))?;

    let response = client.get_keccak_inclusion(&blob_id).await?;

    dbg!("{:?}", response);
    Ok(())
}
