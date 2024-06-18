use anyhow::Result;
use bytes::Bytes;
use futures::prelude::*;
use kv::{CommandRequest, RocksDB, Service, ServiceInner};
use prost::Message;
use tokio::net::TcpListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let service: Service<RocksDB> = ServiceInner::new(RocksDB::new("/tmp/kvserver"))
        .fn_before_send(|res| match res.message.as_ref() {
            "" => res.message = "altered. Original message is empty.".into(),
            s => res.message = format!("altered: {}", s),
        })
        .into();
    let addr = "127.0.0.1:9527";
    let listener = TcpListener::bind(addr).await?;
    info!("Start listening on {}", addr);
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Client {:?} connected", addr);
        let svc = service.clone();
        // 使用默认 4 字节长度作为消息帧的头部
        let mut stream = Framed::new(stream, LengthDelimitedCodec::new());
        tokio::spawn(async move {
            while let Some(Ok(cmd)) = stream.next().await {
                let cmd = CommandRequest::decode(cmd).unwrap();
                info!("Got a new command: {:?}", cmd);
                let res = svc.execute(cmd);
                stream.send(Bytes::from(res.encode_to_vec())).await.unwrap();
            }
            info!("Client {:?} disconnected", addr);
        });
    }
}
