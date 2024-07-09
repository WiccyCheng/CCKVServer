use ::anyhow::Result;
use clap::{Parser, ValueEnum};
use kv::{
    ClientConfig, ClientSecurityProtocol, ClientTlsConfig, GeneralConfig, LogConfig, NetworkType,
    RotationConfig, ServerConfig, ServerSecurityProtocol, ServerTlsConfig, StorageConfig,
    TLS_CA_CERT, TLS_CLIENT_CERT, TLS_CLIENT_KEY, TLS_SERVER_CERT, TLS_SERVER_KEY,
};
use std::{env, fs};

#[derive(Debug, Parser)]
#[clap(
    name = "Server & Client Config Generator",
    about = "help generating client and server config"
)]
struct Args {
    #[clap(short, long, value_enum)]
    protocol: Protocol,
}

#[derive(Debug, ValueEnum, Clone)]
enum Protocol {
    Tls,
    Noise,
    Quic,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let dir = env::current_dir().expect("Failed to get current directory");

    let security_type = args.protocol;

    let general_config = GeneralConfig {
        addr: "127.0.0.1:1973".into(),
        network: match security_type {
            Protocol::Tls => NetworkType::Tcp,
            Protocol::Noise => NetworkType::Tcp,
            Protocol::Quic => NetworkType::Quic,
        },
    };

    let (s_security, c_security) = gen_security_protocol(&security_type);
    let (s_conf_path, c_conf_path) = match security_type {
        Protocol::Tls => (
            dir.join("fixtures/tls/server.conf"),
            dir.join("fixtures/tls/client.conf"),
        ),
        Protocol::Noise => (
            dir.join("fixtures/noise/server.conf"),
            dir.join("fixtures/noise/client.conf"),
        ),
        Protocol::Quic => (
            dir.join("fixtures/quic/server.conf"),
            dir.join("fixtures/quic/client.conf"),
        ),
    };
    println!("server.conf will be placed at: {s_conf_path:?}");
    println!("client.conf will be placed at: {c_conf_path:?}");

    let server_config = ServerConfig {
        storage: StorageConfig::MemTable,
        general: general_config.clone(),
        security: s_security,
        log: LogConfig {
            enable_jaeger: false,
            enable_log_file: false,
            log_level: "info".to_string(),
            path: "/tmp/kv-log".into(),
            rotation: RotationConfig::Daily,
        },
    };

    fs::write(s_conf_path, toml::to_string_pretty(&server_config)?)?;

    let client_config = ClientConfig {
        general: general_config,
        security: c_security,
    };

    fs::write(c_conf_path, toml::to_string_pretty(&client_config)?)?;

    Ok(())
}

fn gen_security_protocol(s: &Protocol) -> (ServerSecurityProtocol, ClientSecurityProtocol) {
    match s {
        Protocol::Tls => (
            ServerSecurityProtocol::Tls(ServerTlsConfig {
                cert: TLS_SERVER_CERT.into(),
                key: TLS_SERVER_KEY.into(),
                ca: Some(TLS_CA_CERT.into()),
            }),
            ClientSecurityProtocol::Tls(ClientTlsConfig {
                identity: Some((TLS_CLIENT_CERT.into(), TLS_CLIENT_KEY.into())),
                ca: Some(TLS_CA_CERT.into()),
                domain: "kvserver.acme.inc".into(),
            }),
        ),
        Protocol::Noise => (ServerSecurityProtocol::Noise, ClientSecurityProtocol::Noise),
        Protocol::Quic => todo!(),
    }
}
