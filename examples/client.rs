use anyhow::Result;
use async_prost::AsyncProstStream;
use futures::prelude::*;
use kv::{CommandRequest, CommandResponse};
use tokio::net::TcpStream;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:9527";

    let stream = TcpStream::connect(addr).await?;

    let mut client =
        AsyncProstStream::<_, CommandResponse, CommandRequest, _>::from(stream).for_async();

    let cmds = vec![
        CommandRequest::new_hset("table", "hello", "world"),
        CommandRequest::new_hset("table", "hello", "a whole new world"),
        CommandRequest::new_hget("table", "hello"),
    ];

    for cmd in cmds {
        client.send(cmd).await?;
        if let Some(Ok(data)) = client.next().await {
            info!("Got response {:?}", data);
        }
    }

    Ok(())
}
