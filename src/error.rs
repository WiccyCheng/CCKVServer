use crate::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KvError {
    #[error("Not found fot table: {0}, key{1}")]
    NotFound(String, String),
    #[error("Frame is larger than max size")]
    FrameError,
    #[error("Command is invalid {0}")]
    InvaildCommand(String),
    #[error("Cannot convert value {0:?} to {1}")]
    ConvertError(Value, &'static str),
    #[error("Cannot process command {command} with table: {table}, key: {key}. Error: {error}")]
    StorageError {
        command: &'static str,
        table: String,
        key: String,
        error: String,
    },

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

    #[error("Internal error: {0}")]
    Internal(String),
}
