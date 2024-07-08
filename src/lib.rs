mod config;
mod error;
mod network;
mod pb;
mod service;
mod storage;

pub use config::*;
pub use error::*;
pub use network::*;
pub use pb::abi::*;
pub use service::*;
pub use storage::*;

use ::anyhow::Result;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::client;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, instrument};

// 通过配置创建 KV 服务器
#[instrument(name = "start_server_with_config", skip_all)]
pub async fn start_server_with_config(config: &ServerConfig) -> Result<()> {
    let acceptor = TlsServerAcceptor::new(
        &config.security.cert,
        &config.security.key,
        config.security.ca.as_deref(),
    )?;

    let addr = &config.general.addr;
    match &config.storage {
        StorageConfig::MemTable => start_tls_server(addr, MemTable::new(), acceptor).await,
        StorageConfig::Sledb(path) => start_tls_server(addr, SledDb::new(path), acceptor).await,
        StorageConfig::Rocksdb(path) => start_tls_server(addr, RocksDB::new(path), acceptor).await,
    }
}

#[instrument(name = "start_client_with_config", skip_all)]
pub async fn start_client_with_config(
    config: &ClientConfig,
) -> Result<YamuxBuilder<client::TlsStream<TcpStream>>> {
    let addr = &config.general.addr;
    let tls = &config.security;

    let identity = tls.identity.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));
    let connector = TlsClientConnector::new(&tls.domain, identity, tls.ca.as_deref())?;
    let stream = TcpStream::connect(addr).await?;
    let stream = connector.connect(stream).await?;

    // 打开一个 stream
    Ok(YamuxBuilder::new_client(stream, None))
}

async fn start_tls_server<Store: Storage>(
    addr: &str,
    store: Store,
    acceptor: TlsServerAcceptor,
) -> Result<()> {
    let service: Service<Store> = ServiceInner::new(store).into();
    let listener = TcpListener::bind(addr).await?;
    info!("Start listening on {addr}");
    loop {
        let tls = acceptor.clone();
        let (stream, addr) = listener.accept().await?;
        info!("Client {addr:?} connected");

        let svc = service.clone();
        tokio::spawn(async move {
            let stream = tls.accept(stream).await.unwrap();
            YamuxBuilder::new_server(stream, None, move |stream| {
                let svc = svc.clone();
                async move {
                    let stream = ProstServerStream::new(stream.compat(), svc.clone());
                    stream.process().await.unwrap();
                    Ok(())
                }
            })
        });
    }
}
