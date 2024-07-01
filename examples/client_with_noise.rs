use anyhow::Result;
use kv::{CommandRequest, NoiseInitiator, ProstClientStream};
use tokio::net::TcpStream;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:9527";

    let stream = TcpStream::connect(addr).await?;

    let stream = NoiseInitiator::connect(stream).await?;

    let mut client = ProstClientStream::new(stream);

    let cmd = CommandRequest::new_hset("table", "key", "value");

    let data = client.execute_unary(&cmd).await?;
    info!("Got response {data:?}");

    Ok(())
}
