use anyhow::Result;
use certify::{generate_ca, generate_cert, CertSigAlgo, CertType, CA};
use tokio::fs;

struct CertPem {
    cert_type: CertType,
    cert: String,
    key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let pem = create_ca()?;
    gen_files(&pem).await?;
    let ca = CA::load(&pem.cert, &pem.key)?;
    let pem = create_cert(&ca, &["kvserver.acme.inc"], "Acme KV server", false)?;
    gen_files(&pem).await?;
    let pem = create_cert(&ca, &[], "awesome-device-id", true)?;
    gen_files(&pem).await?;
    Ok(())
}

fn create_ca() -> Result<CertPem> {
    let (cert, key) = generate_ca(
        "CN",
        "Acme Inc.",
        "Acme CA",
        // 注意 s2n_quic 库的 tls 不支持 ED25519，需要用 EcDsa 生成，否则无法解析
        CertSigAlgo::EcDsa,
        None,
        Some(10 * 365),
    )?;
    Ok(CertPem {
        cert_type: CertType::CA,
        cert,
        key,
    })
}

// domain 用于设置证书的 "Server Name Indication" (SNI) 值，客户端在 TLS 握手
// 期间，可以通过 with_server_name 来指定期望的主机名，以此来防止中间人攻击
fn create_cert(ca: &CA, domains: &[&str], cn: &str, is_client: bool) -> Result<CertPem> {
    let (days, cert_type) = if is_client {
        (Some(365), CertType::Client)
    } else {
        (Some(5 * 365), CertType::Server)
    };
    let (cert, key) = generate_cert(
        ca,
        domains.to_vec(),
        "CN",
        "Acme Inc.",
        cn,
        // 注意 s2n_quic 库的 tls 不支持 ED25519，需要用 EcDsa 生成，否则无法解析
        CertSigAlgo::EcDsa,
        None,
        is_client,
        days,
    )?;
    Ok(CertPem {
        cert_type,
        cert,
        key,
    })
}

async fn gen_files(pem: &CertPem) -> Result<()> {
    let name = match pem.cert_type {
        CertType::Client => "client",
        CertType::Server => "server",
        CertType::CA => "ca",
    };
    fs::write(format!("fixtures/{}.cert", name), pem.cert.as_bytes()).await?;
    fs::write(format!("fixtures/{}.key", name), pem.key.as_bytes()).await?;
    Ok(())
}
