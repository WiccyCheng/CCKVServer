use crate::KvError;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{fs, str::FromStr};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    pub general: GeneralConfig,
    pub storage: StorageConfig,
    pub security: ServerSecurityProtocol,
    pub log: LogConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClientConfig {
    pub general: GeneralConfig,
    pub security: ClientSecurityProtocol,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ServerSecurityProtocol {
    Tls(ServerTlsConfig),
    Noise,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ClientSecurityProtocol {
    Tls(ClientTlsConfig),
    Noise,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LogConfig {
    pub enable_log_file: bool,
    pub enable_jaeger: bool,
    pub log_level: String,
    pub path: String,
    pub rotation: RotationConfig,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            enable_log_file: false,
            enable_jaeger: false,
            log_level: "info".to_string(),
            path: "/tmp/kv-log".into(),
            rotation: RotationConfig::Daily,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ValueEnum)]
pub enum RotationConfig {
    Hourly,
    Daily,
    Never,
}

impl Default for RotationConfig {
    fn default() -> Self {
        RotationConfig::Daily
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    pub addr: String,
    #[serde(default)]
    pub network: NetworkType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkType {
    Tcp,
    Quic,
}

impl Default for NetworkType {
    fn default() -> Self {
        NetworkType::Tcp
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum StorageConfig {
    MemTable,
    Sledb(String),
    Rocksdb(String),
}

impl FromStr for StorageConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "memtable" => Ok(StorageConfig::MemTable),
            "sledb" => Ok(StorageConfig::Sledb("tmp/sledb".to_string())), // Adjust the path as needed
            "rocksdb" => Ok(StorageConfig::Rocksdb("tmp/rocksdb".to_string())), // Adjust the path as needed
            _ => Err(format!("'{}' is not a valid value for StorageConfig", s)),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig::MemTable
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerTlsConfig {
    pub cert: String,
    pub key: String,
    pub ca: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClientTlsConfig {
    pub domain: String,
    pub identity: Option<(String, String)>,
    pub ca: Option<String>,
}

impl ServerConfig {
    pub fn load(path: &str) -> Result<Self, KvError> {
        let config = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&config)?;
        Ok(config)
    }
}

impl ClientConfig {
    pub fn load(path: &str) -> Result<Self, KvError> {
        let config = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use crate::{TLS_CLIENT_CONFIG, TLS_SERVER_CONFIG};

    use super::*;

    #[test]
    fn server_config_should_be_loaded() {
        let result: Result<ServerConfig, toml::de::Error> = toml::from_str(TLS_SERVER_CONFIG);
        assert!(result.is_ok())
    }

    #[test]
    fn client_config_should_be_loaded() {
        let result: Result<ClientConfig, toml::de::Error> = toml::from_str(TLS_CLIENT_CONFIG);
        assert!(result.is_ok())
    }
}
