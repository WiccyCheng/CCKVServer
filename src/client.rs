use anyhow::Result;
use futures::StreamExt;
use kv::{CommandRequest, KvError, ProstClientStream, TlsClientConnector, YamuxBuilder};
use std::time::Duration;
use tokio::{net::TcpStream, time};
use tokio_util::compat::Compat;
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
    let mut connection = YamuxBuilder::new_client(stream, None);

    let channel = "lobby";
    start_publishing(connection.open_stream().await?, &channel)?;

    let stream = connection.open_stream().await?;
    let mut client = ProstClientStream::new(stream);

    let cmd = CommandRequest::new_hset("table", "key", "value");
    let data = client.execute_unary(&cmd).await?;
    info!("Got response {data:?}");

    // 生成一个 Subscribe 命令
    let cmd = CommandRequest::new_subscribe(channel);
    let mut stream = client.execute_streaming(&cmd).await?;
    let id = stream.id;
    start_unsubscribe(connection.open_stream().await?, &channel, id)?;

    while let Some(Ok(data)) = stream.next().await {
        println!("Got published data: {data:?}");
    }

    Ok(())
}

pub fn start_publishing(stream: Compat<yamux::Stream>, name: &str) -> Result<(), KvError> {
    let cmd = CommandRequest::new_publish(name, vec![1.into(), 2.into(), "hello".into()]);
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(1000)).await;
        let mut client = ProstClientStream::new(stream);
        let res = client.execute_unary(&cmd).await.unwrap();
        println!("Finished publishing: {res:?}");
    });

    Ok(())
}

pub fn start_unsubscribe(
    stream: Compat<yamux::Stream>,
    name: &str,
    id: u32,
) -> Result<(), KvError> {
    let cmd = CommandRequest::new_unsubscribe(name, id);
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(1000)).await;
        let mut client = ProstClientStream::new(stream);
        let res = client.execute_unary(&cmd).await.unwrap();
        println!("Finished unsubscribing: {res:?}");
    });

    Ok(())
}
