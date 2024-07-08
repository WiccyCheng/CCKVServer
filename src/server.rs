use std::env;

use anyhow::Result;
use kv::{start_server_with_config, RotationConfig, ServerConfig};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace, Resource};
use tokio::fs;
use tracing::span;
use tracing_subscriber::{
    fmt::{self, format},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 如果有环境变量，使用环境变量中的 config
    let config = match env::var("KV_SERVER_CONFIG") {
        Ok(path) => fs::read_to_string(&path).await?,
        Err(_) => include_str!("../fixtures/server.conf").to_string(),
    };
    let config: ServerConfig = toml::from_str(&config)?;

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
    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // 添加
    let log = &config.log;
    let file_appender = match log.rotation {
        RotationConfig::Hourly => tracing_appender::rolling::hourly(&log.path, "server.log"),
        RotationConfig::Daily => tracing_appender::rolling::daily(&log.path, "server.log"),
        RotationConfig::Never => tracing_appender::rolling::never(&log.path, "server.log"),
    };

    let (non_blocking, _guard1) = tracing_appender::non_blocking(file_appender);
    let fmt_layer = fmt::layer()
        .event_format(format().compact())
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt_layer)
        .with(opentelemetry)
        .init();

    let root = span!(tracing::Level::INFO, "app_start", work_units = 2);
    let _enter = root.enter();

    start_server_with_config(&config).await?;

    Ok(())
}
