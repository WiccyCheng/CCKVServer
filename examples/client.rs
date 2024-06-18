use anyhow::Result;
use bytes::Bytes;
use futures::prelude::*;
use kv::{CommandRequest, CommandResponse};
use prost::Message;
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:9527";

    let stream = TcpStream::connect(addr).await?;

    let mut client = Framed::new(stream, LengthDelimitedCodec::new());

    let cmds = vec![
        CommandRequest::new_hset("table", "hello", "world"),
        CommandRequest::new_hset("table", "hello", "a whole new world"),
        CommandRequest::new_hget("table", "hello"),
    ];

    for cmd in cmds {
        client.send(Bytes::from(cmd.encode_to_vec())).await?;
        if let Some(Ok(data)) = client.next().await {
            let data = CommandResponse::decode(data).unwrap();
            info!("Got response {:?}", data);
        }
    }

    Ok(())
}
