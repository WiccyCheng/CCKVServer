use anyhow::Result;
use kv::{MemTable, NoiseResponder, ProstServerStream, Service, ServiceInner};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let addr = "127.0.0.1:9527";

    let service: Service = ServiceInner::new(MemTable::new()).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Starting listening on {addr}");
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Client {addr:?} connected");
        let stream = NoiseResponder::accept(stream).await?;
        let stream = ProstServerStream::new(stream, service.clone());
        tokio::spawn(async move { stream.process().await });
    }
}
