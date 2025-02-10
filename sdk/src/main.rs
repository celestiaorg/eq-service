use eq_sdk::EqClient;
use log::{debug, error, info};
use tonic::transport::Endpoint;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service_socket =
        "http://".to_string() + &std::env::var("EQ_SOCKET").expect("EQ_SOCKET env var required");
    dbg!(&service_socket);
    let channel = Endpoint::from_shared(service_socket)?.connect().await?;
    let client = EqClient::new(channel);

    Err("todo".into())
}
