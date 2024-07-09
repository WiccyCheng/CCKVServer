use std::{env, str::FromStr};

use anyhow::Result;
use kv::{start_server_with_config, RotationConfig, ServerConfig};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace, Resource};
use tokio::fs;
use tracing::span;
use tracing_subscriber::{
    filter,
    fmt::{self, format},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 如果有环境变量，使用环境变量中的 config
    let config = match env::var("KV_SERVER_CONFIG") {
        Ok(path) => fs::read_to_string(&path).await?,
        Err(_) => include_str!("../fixtures/quic_server.conf").to_string(),
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

    // log
    let log = &config.log;
    env::set_var("RUST_LOG", &log.log_level);
    let file_appender = match log.rotation {
        RotationConfig::Hourly => tracing_appender::rolling::hourly(&log.path, "server.log"),
        RotationConfig::Daily => tracing_appender::rolling::daily(&log.path, "server.log"),
        RotationConfig::Never => tracing_appender::rolling::never(&log.path, "server.log"),
    };
    let stdout_log = fmt::layer().compact();
    let level = filter::LevelFilter::from_str(&log.log_level)?;
    let jaeger_level = match log.enable_log_file {
        true => level,
        false => filter::LevelFilter::OFF,
    };
    let log_file_level = match log.enable_log_file {
        true => level,
        false => filter::LevelFilter::OFF,
    };
    let (non_blocking, _guard1) = tracing_appender::non_blocking(file_appender);
    let fmt_layer = fmt::layer()
        .event_format(format().compact())
        .with_writer(non_blocking);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(stdout_log)
        .with(fmt_layer.with_filter(log_file_level))
        .with(opentelemetry.with_filter(jaeger_level))
        .init();

    let root = span!(tracing::Level::INFO, "app_start", work_units = 2);
    let _enter = root.enter();

    start_server_with_config(&config).await?;

    Ok(())
}
