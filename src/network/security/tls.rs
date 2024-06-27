use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::server::WebPkiClientVerifier;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::{client::TlsStream as ClientTlsStream, server::TlsStream as ServerTlsStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::{ClientSecurityStream, KvError, ServerSecurityStream};
use std::io::Cursor;
use std::sync::Arc;

/// KV Server 自己的 ALPN (Application-Layer Protocol Negotiation)
const ALPN_KV: &str = "kv";

/// 存放 TLS ServerConfig 并提供方法 accept 把底层的协议转换成 TLS
#[derive(Clone)]
pub struct TlsServerAcceptor {
    inner: Arc<ServerConfig>,
}

/// 存放 TLS Client 并提供方法 connect 把底层的协议转换成 TLS
#[derive(Clone)]
pub struct TlsClientConnector {
    pub config: Arc<ClientConfig>,
    pub domain: Arc<String>,
}

pub struct TlsStream;

impl TlsClientConnector {
    /// 加载 client cert / CA cert，生成 ClientConfig
    /// server_ca 选项应传递根证书
    pub fn new(
        domain: impl Into<String>,
        identity: Option<(&str, &str)>,
        // 在 TLS（传输层安全性）协议中，server_ca 选项是用于指定服务器证书的信任链根证书（CA 证书），而不是服务器证书本身。
        // 这是因为客户端需要验证服务器提供的证书是否可信，而这种验证通常是通过一个或多个根证书（CA 证书）来完成的。
        // 传递根证书而不是服务器证书，目的是让客户端能够信任由该 CA 颁发的所有证书。
        server_ca: Option<&str>,
    ) -> Result<Self, KvError> {
        let mut root_cert_store = RootCertStore::empty();
        // 加载本地信任的根证书链
        for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs")
        {
            root_cert_store.add(cert)?;
        }

        // 如果有签署服务器的 CA 证书，则加载它，这样服务器证书不在根证书链
        // 但是这个 CA 证书能验证它，也可以
        if let Some(server_ca) = server_ca {
            root_cert_store.add_parsable_certificates(load_certs(server_ca)?);
        }

        let config = match identity {
            Some((cert, key)) => {
                let certs = load_certs(cert)?;
                let key = load_key(key)?;
                ClientConfig::builder()
                    .with_root_certificates(root_cert_store)
                    .with_client_auth_cert(
                        certs.into_iter().map(|cert| cert.into_owned()).collect(),
                        key.clone_key(),
                    )?
            }
            None => ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth(),
        };

        Ok(Self {
            config: Arc::new(config),
            domain: Arc::new(domain.into()),
        })
    }
}

impl TlsServerAcceptor {
    /// 加载 server cert / CA cert，生成 ServerConfig
    /// client_ca 不为空时将验证客户端证书
    pub fn new(cert: &str, key: &str, client_ca: Option<&str>) -> Result<Self, KvError> {
        let certs = load_certs(cert)?
            .into_iter()
            .map(|cert| cert.into_owned())
            .collect();
        let key = load_key(key)?.clone_key();

        let config = match client_ca {
            None => ServerConfig::builder().with_no_client_auth(),
            Some(cert) => {
                // 如果客户端证书是某个 CA 证书签发的，则把这个 CA 证书加载到信任链中
                let mut client_root_cert_store = RootCertStore::empty();
                client_root_cert_store.add_parsable_certificates(load_certs(cert)?);
                let client_auth = WebPkiClientVerifier::builder(client_root_cert_store.into())
                    // 允许无证书的客户端链接
                    // .allow_unauthenticated()
                    .build()
                    .map_err(|_| KvError::CertifcateParseError("server", "cert verifier"))?;
                ServerConfig::builder().with_client_cert_verifier(client_auth)
            }
        };

        // 加载服务器证书
        let mut config = config
            .with_single_cert(certs, key)
            .map_err(|_| KvError::CertifcateParseError("server", "cert"))?;
        config.alpn_protocols = vec![Vec::from(ALPN_KV)];

        Ok(Self {
            inner: Arc::new(config),
        })
    }
}

impl ClientSecurityStream for TlsClientConnector {
    type Stream<S> = ClientTlsStream<S>;

    /// 触发 TLS 协议，把底层的 stream 转换成 TLS stream
    async fn connect<S>(&self, stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send,
    {
        let dns = ServerName::try_from(self.domain.as_str())
            .map_err(|_| KvError::Internal("Invalid DNS name".to_string()))?;

        let stream = TlsConnector::from(self.config.clone())
            .connect(dns.to_owned(), stream)
            .await?;

        Ok(stream)
    }
}

impl ServerSecurityStream for TlsServerAcceptor {
    type Stream<S> = ServerTlsStream<S>;

    async fn accept<S>(&self, stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send,
    {
        let acceptor = TlsAcceptor::from(self.inner.clone());
        Ok(acceptor.accept(stream).await?)
    }
}

fn load_certs(cert: &str) -> Result<Vec<CertificateDer>, KvError> {
    let mut cert = Cursor::new(cert);
    rustls_pemfile::certs(&mut cert)
        .map(|cert| cert.map_err(|e| e.into()))
        .collect()
}

fn load_key(key: &str) -> Result<PrivateKeyDer, KvError> {
    let mut cursor = Cursor::new(key);

    // PKCS#8 是一种标准的私钥信息语法，支持多种加密算法。它可以包含 RSA、DSA、ECDSA 等各种类型的私钥。
    // 使用 PKCS#8 格式加载私钥可以处理不同类型的私钥，因此优先尝试这种格式。
    if let Some(key) = rustls_pemfile::pkcs8_private_keys(&mut cursor)
        .into_iter()
        // Result::ok 是方法指针，此处直接传递给filter_map，由filter_map来调用
        .filter_map(Result::ok)
        .next()
    {
        // 每个密钥文件通常只包含一个私钥（或者至少只有一个是当前需要的有效密钥）。加载第一个有效的私钥就足够了
        return Ok(key.into());
    }

    // 再尝试加载 RSA key
    cursor.set_position(0);
    if let Some(key) = rustls_pemfile::rsa_private_keys(&mut cursor)
        .into_iter()
        .filter_map(Result::ok)
        .next()
    {
        return Ok(key.into());
    }

    // 不支持的私钥类型
    Err(KvError::CertifcateParseError("private", "key"))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;
    use anyhow::Result;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    const CA_CERT: &str = include_str!("../../../fixtures/ca.cert");
    const CLIENT_CERT: &str = include_str!("../../../fixtures/client.cert");
    const CLIENT_KEY: &str = include_str!("../../../fixtures/client.key");
    const SERVER_CERT: &str = include_str!("../../../fixtures/server.cert");
    const SERVER_KEY: &str = include_str!("../../../fixtures/server.key");

    #[tokio::test]
    async fn tls_should_work() -> Result<()> {
        let ca = Some(CA_CERT);

        let addr = start_server(None).await?;

        let connector = TlsClientConnector::new("kvserver.acme.inc", None, ca)?;
        let stream = TcpStream::connect(addr).await?;
        let mut stream = connector.connect(stream).await?;
        stream.write_all(b"hello world!").await?;
        let mut buf = [0; 12];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"hello world!");

        Ok(())
    }

    #[tokio::test]
    async fn tls_with_client_cert_should_work() -> Result<()> {
        let client_identity = Some((CLIENT_CERT, CLIENT_KEY));
        let ca = Some(CA_CERT);

        let addr = start_server(ca).await?;

        let connector = TlsClientConnector::new("kvserver.acme.inc", client_identity, ca)?;
        let stream = TcpStream::connect(addr).await?;
        let mut stream = connector.connect(stream).await?;
        stream.write_all(b"hello world!").await?;
        let mut buf = [0; 12];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"hello world!");

        Ok(())
    }

    #[tokio::test]
    async fn tls_with_bad_domain_should_not_work() -> Result<()> {
        let addr = start_server(None).await?;

        let connector = TlsClientConnector::new("kvserver1.acme.inc", None, Some(CA_CERT))?;
        let srteam = TcpStream::connect(addr).await?;
        let result = connector.connect(srteam).await;

        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn tls_with_client_has_no_cert_should_not_work() -> Result<()> {
        let addr = start_server(Some(CA_CERT)).await?;

        let connector = TlsClientConnector::new("kvserver.acme.inc", None, Some(CA_CERT))?;
        let stream = TcpStream::connect(addr).await.unwrap();
        // 开始tls握手，由于tls握手是异步操作，此时tls握手一般还未完成
        let mut stream = connector.connect(stream).await.unwrap();

        let mut buf = [0; 12];
        // 对stream进行数据处理时，我们可以确定tls握手已经完成
        let result = stream.read_exact(&mut buf).await;
        assert!(result.is_err());

        Ok(())
    }

    async fn start_server(ca: Option<&str>) -> Result<SocketAddr> {
        let acceptor = TlsServerAcceptor::new(SERVER_CERT, SERVER_KEY, ca)?;

        let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = echo.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = echo.accept().await.unwrap();
            // 在开启客户端验证的情况下，客户端无证书时建连失败
            if let Ok(mut stream) = acceptor.accept(stream).await {
                let mut buf = [0; 12];
                stream.read_exact(&mut buf).await.unwrap();
                stream.write_all(&buf).await.unwrap();
            }
        });

        Ok(addr)
    }
}
