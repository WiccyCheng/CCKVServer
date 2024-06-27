use anyhow::Result;
use kv::{ClientSecurityStream, CommandRequest, ProstClientStream, TlsClientConnector};
use tokio::net::TcpStream;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:9527";

    let ca_cert = Some(include_str!("../fixtures/ca.cert"));
    let client_identity = Some((
        include_str!("../fixtures/client.cert"),
        include_str!("../fixtures/client.key"),
    ));

    let connector = TlsClientConnector::new("kvserver.acme.inc", client_identity, ca_cert)?;

    let stream = TcpStream::connect(addr).await?;

    let stream = connector.connect(stream).await?;

    let mut client = ProstClientStream::new(stream);

    let cmd = CommandRequest::new_hset("table", "key", "value");

    let data = client.execute(cmd).await?;
    info!("Got response {data:?}");

    Ok(())
}
