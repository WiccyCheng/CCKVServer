use std::fs;

use ::anyhow::Result;
use kv::{
    ClientConfig, ClientTlsConfig, GeneralConfig, LogConfig, RotationConfig, ServerConfig,
    ServerTlsConfig,
};
fn main() -> Result<()> {
    const CA_CERT: &str = include_str!("../fixtures/ca.cert");
    const SERVER_CERT: &str = include_str!("../fixtures/server.cert");
    const SERVER_KEY: &str = include_str!("../fixtures/server.key");

    let general_config = GeneralConfig {
        addr: "127.0.0.1:1973".into(),
        network: kv::NetworkType::Tcp,
    };

    let server_config = ServerConfig {
        storage: kv::StorageConfig::MemTable,
        general: general_config.clone(),
        security: ServerTlsConfig {
            cert: SERVER_CERT.into(),
            key: SERVER_KEY.into(),
            ca: Some(CA_CERT.into()),
        },
        log: LogConfig {
            enable_jaeger: false,
            enable_log_file: false,
            log_level: "info".to_string(),
            path: "/tmp/kv-log".into(),
            rotation: RotationConfig::Daily,
        },
    };

    fs::write(
        "fixtures/server.conf",
        toml::to_string_pretty(&server_config)?,
    )?;

    const CLIENT_CERT: &str = include_str!("../fixtures/client.cert");
    const CLIENT_KEY: &str = include_str!("../fixtures/client.key");

    let client_config = ClientConfig {
        general: general_config,
        security: ClientTlsConfig {
            identity: Some((CLIENT_CERT.into(), CLIENT_KEY.into())),
            ca: Some(CA_CERT.into()),
            domain: "kvserver.acme.inc".into(),
        },
    };

    fs::write(
        "fixtures/client.conf",
        toml::to_string_pretty(&client_config)?,
    )?;

    Ok(())
}
