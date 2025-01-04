use opentelemetry::{global, sdk::trace::Config};
use std::error::Error;
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub fn init_telemetry() -> Result<(), Box<dyn Error>> {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(Config::default())
        .install_batch(opentelemetry::runtime::Tokio)?;

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = Registry::default()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry);

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

pub fn shutdown_telemetry() {
    global::shutdown_tracer_provider();
}
