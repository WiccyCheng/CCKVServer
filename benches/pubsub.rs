use anyhow::Result;
use criterion::{criterion_group, criterion_main, Criterion};
use futures::StreamExt;
use kv::{
    start_server_with_config, start_yamux_client_with_config, AppStream, ClientConfig,
    CommandRequest, ServerConfig, StorageConfig, YamuxConn,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace, Resource};
use rand::seq::SliceRandom;
use std::time::Duration;
use tokio::{net::TcpStream, runtime::Builder, time};
use tokio_rustls::client::TlsStream;
use tracing::{info, span};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

async fn start_server() -> Result<()> {
    let addr = "127.0.0.1:1973";
    let mut config: ServerConfig = toml::from_str(include_str!("../fixtures/server.conf"))?;
    config.general.addr = addr.into();
    config.storage = StorageConfig::MemTable;

    tokio::spawn(async move {
        start_server_with_config(&config).await.unwrap();
    });

    Ok(())
}

async fn connect() -> Result<YamuxConn<TlsStream<TcpStream>>> {
    let addr = "127.0.0.1:1973";
    let mut config: ClientConfig = toml::from_str(include_str!("../fixtures/client.conf"))?;
    config.general.addr = addr.into();

    Ok(start_yamux_client_with_config(&config).await?)
}

async fn start_subscribers(topic: &'static str) -> Result<()> {
    let mut connection = connect().await?;
    let client = connection.open_stream().await?;
    info!("C(subscriber): stream opened");
    let cmd = CommandRequest::new_subscribe(topic.to_string());
    tokio::spawn(async move {
        let mut stream = client.execute_streaming(&cmd).await.unwrap();
        while let Some(Ok(data)) = stream.next().await {
            drop(data);
        }
    });

    Ok(())
}

async fn start_publishers(topic: &'static str, values: &'static [&'static str]) -> Result<()> {
    let mut rng = rand::thread_rng();
    let v = values.choose(&mut rng).unwrap();

    let mut connection = connect().await?;
    let mut client = connection.open_stream().await?;
    info!("C(publishers): stream opened");

    let cmd = CommandRequest::new_publish(topic.to_string(), vec![(*v).into()]);
    client.execute_unary(&cmd).await.unwrap();

    Ok(())
}

fn pubsub(c: &mut Criterion) {
    // 创建 Tokio runtime
    let runtime = Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("pubsub")
        .enable_all()
        .build()
        .unwrap();

    let values = &["Hello", "Wiccy", "Goodbye", "World"];
    let topic = "lobby";

    // 运行服务器和 100 个 subscriber，为测试做准备
    runtime.block_on(async {
        // 设置 OpenTelemetry 管道
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint("http://localhost:4317"),
            )
            .with_trace_config(trace::config().with_resource(Resource::new(vec![
                opentelemetry::KeyValue::new("service.name", "kv-bench"),
            ])))
            // install_batch will panic if not called within a tokio runtime
            .install_batch(runtime::Tokio)
            .expect("Error initializing tracer");

        // 创建 tracing-opentelemetry layer 并将其与 Tracer 结合使用
        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

        // 设置 tracing 订阅者
        let subscriber = Registry::default()
            .with(telemetry)
            .with(EnvFilter::from_default_env());
        tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

        let root = span!(tracing::Level::INFO, "app_start", work_units = 2);
        let _enter = root.enter();

        eprint!("preparing server and subscribers");
        start_server().await.unwrap();
        time::sleep(Duration::from_millis(50)).await;
        for _ in 0..100 {
            start_subscribers(topic).await.unwrap();
            eprint!(".");
        }
        eprintln!("Done");
    });

    // 进行 benchmark
    c.bench_function("publishing", move |b| {
        b.to_async(&runtime)
            .iter(|| async { start_publishers(topic, values).await })
    });
}
criterion_group! {name = benches;
config = Criterion::default().measurement_time(Duration::new(10, 0)) // 增加目标时间
.sample_size(10); // 设置样本大小
targets = pubsub}
criterion_main!(benches);
