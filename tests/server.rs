use anyhow::Result;
use futures::StreamExt;
use kv::{
    start_quic_client_with_config, start_server_with_config, start_yamux_client_with_noise_config,
    start_yamux_client_with_tls_config, AppStream, ClientConfig, CommandRequest, KvError,
    ProstClientStream, NOISE_CLIENT_CONFIG, NOISE_SERVER_CONFIG, QUIC_CLIENT_CONFIG,
    QUIC_SERVER_CONFIG, TLS_CLIENT_CONFIG, TLS_SERVER_CONFIG,
};
use std::time::Duration;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time,
};
use tracing::info;

#[tokio::test]
async fn quic_server_client_full_tests() -> Result<()> {
    // 启动服务器
    let server_config = toml::from_str(QUIC_SERVER_CONFIG)?;
    tokio::spawn(async move {
        start_server_with_config(&server_config).await.unwrap();
    });

    let config: ClientConfig = toml::from_str(QUIC_CLIENT_CONFIG)?;
    let conn = start_quic_client_with_config(&config).await?;
    process(conn).await?;

    Ok(())
}

#[tokio::test]
async fn yamux_server_client_full_tests() -> Result<()> {
    // 启动服务器
    let server_config = toml::from_str(TLS_SERVER_CONFIG)?;
    tokio::spawn(async move {
        start_server_with_config(&server_config).await.unwrap();
    });

    let config: ClientConfig = toml::from_str(TLS_CLIENT_CONFIG)?;
    let conn = start_yamux_client_with_tls_config(&config).await?;
    process(conn).await?;

    Ok(())
}

// TODO(Wiccy): Currently noise can not work with yamux, so skip this
// #[tokio::test]
#[allow(dead_code)]
async fn noise_server_client_full_tests() -> Result<()> {
    // 启动服务器
    let server_config = toml::from_str(NOISE_SERVER_CONFIG)?;
    tokio::spawn(async move {
        start_server_with_config(&server_config).await.unwrap();
    });

    let config: ClientConfig = toml::from_str(NOISE_CLIENT_CONFIG)?;
    let conn = start_yamux_client_with_noise_config(&config).await?;

    process(conn).await?;

    Ok(())
}

async fn process<S, T>(mut conn: S) -> Result<()>
where
    S: AppStream<InnerStream = T>,
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let channel = "lobby";
    start_publishing(conn.open_stream().await?, channel)?;

    let mut client = conn.open_stream().await?;

    // 生成一个 HSET 命令
    let cmd = CommandRequest::new_hset("table", "hello", "world");

    // 发送 HSET 命令
    let data = client.execute_unary(&cmd).await?;
    info!("Got response {:?}", data);

    // 生成一个 Subscribe 命令
    let cmd = CommandRequest::new_subscribe(channel);
    let mut stream = conn.open_stream().await?.execute_streaming(&cmd).await?;
    let id = stream.id;
    start_unsubscribe(conn.open_stream().await?, channel, id)?;

    while let Some(Ok(data)) = stream.next().await {
        println!("Got published data: {:?}", data);
    }

    // 生成一个 HGET 命令
    let cmd = CommandRequest::new_hget("table", "hello");
    let data = client.execute_unary(&cmd).await?;

    assert_eq!(data.status, 200);
    assert_eq!(data.values, &["world".into()]);

    Ok(())
}

fn start_publishing<S>(mut stream: ProstClientStream<S>, name: &str) -> Result<(), KvError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let cmd = CommandRequest::new_publish(name, vec![1.into(), 2.into(), "hello".into()]);
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(1000)).await;
        let res = stream.execute_unary(&cmd).await.unwrap();
        println!("Finished publishing: {:?}", res);
    });

    Ok(())
}

fn start_unsubscribe<S>(
    mut stream: ProstClientStream<S>,
    name: &str,
    id: u32,
) -> Result<(), KvError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let cmd = CommandRequest::new_unsubscribe(name, id as _);
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(2000)).await;
        let res = stream.execute_unary(&cmd).await.unwrap();
        println!("Finished unsubscribing: {:?}", res);
    });

    Ok(())
}
