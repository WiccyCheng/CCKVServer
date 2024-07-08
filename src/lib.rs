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
use anyhow::anyhow;
use s2n_quic::{client::Connect, Client, Server};
use std::{net::SocketAddr, str::FromStr};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::client;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{info, instrument, span};

// 通过配置创建 KV 服务器
#[instrument(name = "start_server_with_config", skip_all)]
pub async fn start_server_with_config(config: &ServerConfig) -> Result<()> {
    let addr = &config.general.addr;
    match config.general.network {
        NetworkType::Tcp => {
            let acceptor = TlsServerAcceptor::new(
                &config.security.cert,
                &config.security.key,
                config.security.ca.as_deref(),
            )?;

            match &config.storage {
                StorageConfig::MemTable => {
                    start_tls_server(addr, MemTable::new(), acceptor).await?
                }
                StorageConfig::Sledb(path) => {
                    start_tls_server(addr, SledDb::new(path), acceptor).await?
                }
                StorageConfig::Rocksdb(path) => {
                    start_tls_server(addr, RocksDB::new(path), acceptor).await?
                }
            };
        }
        NetworkType::Quic => {
            match &config.storage {
                StorageConfig::MemTable => {
                    start_quic_server(addr, MemTable::new(), &config.security).await?
                }
                StorageConfig::Sledb(path) => {
                    start_quic_server(addr, SledDb::new(path), &config.security).await?
                }
                StorageConfig::Rocksdb(path) => {
                    start_quic_server(addr, RocksDB::new(path), &config.security).await?
                }
            };
        }
    }

    Ok(())
}

#[instrument(name = "start_yamux_client_with_config", skip_all)]
pub async fn start_yamux_client_with_config(
    config: &ClientConfig,
) -> Result<YamuxConn<client::TlsStream<TcpStream>>> {
    let addr = &config.general.addr;
    let tls = &config.security;

    let identity = tls.identity.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));
    let connector = TlsClientConnector::new(&tls.domain, identity, tls.ca.as_deref())?;
    let stream = TcpStream::connect(addr).await?;
    let stream = connector.connect(stream).await?;

    // 打开一个 stream
    Ok(YamuxConn::new_client(stream, None))
}

#[instrument(name = "start_quic_client_with_config", skip_all)]
pub async fn start_quic_client_with_config(config: &ClientConfig) -> Result<QuicConn> {
    let addr = SocketAddr::from_str(&config.general.addr)?;
    let tls = &config.security;

    let client = Client::builder()
        .with_tls(tls.ca.as_ref().unwrap().as_str())?
        .with_io("0.0.0.0:0")?
        .start()
        .map_err(|e| anyhow!("Failed to start client. Error: {e}"))?;

    let connect = Connect::new(addr).with_server_name("localhost");
    let mut conn = client.connect(connect).await?;

    conn.keep_alive(true)?;

    Ok(QuicConn::new(conn))
}

pub async fn start_quic_server<Store: Storage>(
    addr: &str,
    store: Store,
    tls_config: &ServerTlsConfig,
) -> Result<()> {
    let service: Service<Store> = ServiceInner::new(store).into();
    let mut listener = Server::builder()
        .with_tls((tls_config.cert.as_str(), tls_config.key.as_str()))?
        .with_io(addr)?
        .start()
        .map_err(|e| anyhow::anyhow!("Failed to start server. Error: {}", e))?;

    info!("Start listening on {addr}");

    loop {
        let root = span!(tracing::Level::INFO, "server_process");
        let _enter = root.enter();

        if let Some(mut conn) = listener.accept().await {
            info!("Client {} connected", conn.remote_addr()?);
            let svc = service.clone();

            tokio::spawn(async move {
                while let Ok(Some(stream)) = conn.accept_bidirectional_stream().await {
                    info!(
                        "Accepted stream from {}",
                        stream.connection().remote_addr()?
                    );

                    let svc = svc.clone();
                    tokio::spawn(async move {
                        let stream = ProstServerStream::new(stream, svc);
                        stream.process().await.unwrap();
                    });
                }
                Ok::<(), anyhow::Error>(())
            });
        }
    }
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
            YamuxConn::new_server(stream, None, move |stream| {
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
