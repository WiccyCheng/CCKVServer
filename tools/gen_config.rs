use ::anyhow::{anyhow, Result};
use clap::{Parser, ValueEnum};
use kv::{
    ClientConfig, ClientSecurityProtocol, ClientTlsConfig, GeneralConfig, LogConfig, NetworkType,
    RotationConfig, ServerConfig, ServerSecurityProtocol, ServerTlsConfig, StorageConfig,
    QUIC_CA_CERT, QUIC_CLIENT_CERT, QUIC_CLIENT_KEY, QUIC_SERVER_CERT, QUIC_SERVER_KEY,
    TLS_CA_CERT, TLS_CLIENT_CERT, TLS_CLIENT_KEY, TLS_SERVER_CERT, TLS_SERVER_KEY,
};
use std::{env, fs};

#[derive(Debug, Parser)]
#[clap(
    name = "Server & Client Config Generator",
    about = "help generating client and server config"
)]
struct Args {
    #[clap(
        short,
        long,
        value_enum,
        help = "Tls files are required when using Quic and Tls.\nYou may use tools/gen_cert.rs to generate these files."
    )]
    protocol: Protocol,

    #[clap(short, long, default_value = "127.0.0.1:1973")]
    addr: String,

    #[clap(long, default_value = "false")]
    enable_jaeger: bool,

    #[clap(long, default_value = "false")]
    enable_log_file: bool,

    #[clap(long, default_value = "info")]
    log_level: String,

    #[clap(long, default_value = "/tmp/kv-log")]
    log_path: String,

    #[clap(long, value_enum, default_value = "daily")]
    log_rotation: RotationConfig,

    #[clap(long, default_value = "memtable")]
    storage: StorageConfig,
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
        addr: args.addr,
        network: match security_type {
            Protocol::Tls => {
                check_tls_files()?;
                NetworkType::Tcp
            }
            Protocol::Quic => {
                check_tls_files()?;
                NetworkType::Quic
            }
            Protocol::Noise => NetworkType::Tcp,
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

    let storage = match args.storage {
        StorageConfig::MemTable => StorageConfig::MemTable,
        StorageConfig::Sledb(_) => StorageConfig::Sledb("tmp/sledb".to_string()), // You can adjust the path as needed
        StorageConfig::Rocksdb(_) => StorageConfig::Rocksdb("tmp/rocksdb".to_string()), // You can adjust the path as needed
    };

    let server_config = ServerConfig {
        storage,
        general: general_config.clone(),
        security: s_security,
        log: LogConfig {
            enable_jaeger: args.enable_jaeger,
            enable_log_file: args.enable_log_file,
            log_level: args.log_level,
            path: args.log_path,
            rotation: args.log_rotation,
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
        Protocol::Quic => (
            ServerSecurityProtocol::Tls(ServerTlsConfig {
                cert: QUIC_SERVER_CERT.into(),
                key: QUIC_SERVER_KEY.into(),
                ca: Some(QUIC_CA_CERT.into()),
            }),
            ClientSecurityProtocol::Tls(ClientTlsConfig {
                identity: Some((QUIC_CLIENT_CERT.into(), QUIC_CLIENT_KEY.into())),
                ca: Some(QUIC_CA_CERT.into()),
                domain: "kvserver.acme.inc".into(),
            }),
        ),
    }
}

fn check_tls_files() -> Result<()> {
    // 提示用户需要的文件
    let required_files = [
        "fixtures/tls/ca.cert",
        "fixtures/tls/client.cert",
        "fixtures/tls/client.key",
        "fixtures/tls/server.cert",
        "fixtures/tls/server.key",
    ];

    let missing_files: Vec<&str> = required_files
        .iter()
        .filter(|&&path| fs::metadata(path).is_err())
        .copied()
        .collect();

    if !missing_files.is_empty() {
        eprintln!("The following required files for tls verification are missing:");
        for file in missing_files {
            eprintln!("- {}", file);
        }
        eprintln!("You may use tools/gen_cert.rs to generate these files.");
        eprintln!("Please make sure these files are present before running the program.");
        return Err(anyhow!("Tls verification files missing."));
    }

    Ok(())
}
