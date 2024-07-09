use crate::KvError;
use serde::{Deserialize, Serialize};
use std::fs;

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum RotationConfig {
    Hourly,
    Daily,
    Never,
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
