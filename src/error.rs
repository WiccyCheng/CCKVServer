use thiserror::Error;

#[derive(Error, Debug)]
pub enum KvError {
    #[error("Not found {0}")]
    NotFound(String),
    #[error("Frame is larger than max size")]
    FrameError,
    #[error("Command is invalid {0}")]
    InvalidCommand(String),
    #[error("Cannot convert value {0} to {1}")]
    ConvertError(String, &'static str),
    #[error("Cannot process command {command} with table: {table}, key: {key}. Error: {error}")]
    StorageError {
        command: &'static str,
        table: String,
        key: String,
        error: String,
    },
    #[error("Certificate parse error: error to load {0} {1}")]
    CertifcateParseError(&'static str, &'static str),

    #[error("Failed to encode protobuf message")]
    EncodeError(#[from] prost::EncodeError),
    #[error("Failed to decode protobuf message")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Failed to access sled db")]
    SeldError(#[from] sled::Error),
    #[error("Failed to access rocksdb")]
    RocksDBError(#[from] rocksdb::Error),
    #[error("I/O error")]
    IoError(#[from] std::io::Error),
    #[error("tls error")]
    TlsError(#[from] tokio_rustls::rustls::Error),
    #[error("noise error")]
    NoiseError(#[from] snow::Error),
    #[error("Yamux Connection error")]
    YamuxConnectionError(#[from] yamux::ConnectionError),
    #[error("Quic Connection error")]
    QuicConnectionError(#[from] s2n_quic::connection::Error),
    #[error("Parse config error")]
    ConfigError(#[from] toml::de::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
