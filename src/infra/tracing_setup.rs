use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Initializes structured tracing logs once per process.
pub fn init_tracing() -> Option<SdkTracerProvider> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("login_base=info,tower_http=info"));

    match build_tracer_provider() {
        Some(provider) => {
            let tracer = provider.tracer("login-base");
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true);
            let _ = Registry::default()
                .with(env_filter)
                .with(otel_layer)
                .with(fmt_layer)
                .try_init();
            tracing::info!("otlp trace export enabled");
            Some(provider)
        }
        None => {
            let fmt_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true);
            let _ = Registry::default().with(env_filter).with(fmt_layer).try_init();
            None
        }
    }
}

fn build_tracer_provider() -> Option<SdkTracerProvider> {
    let endpoint =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().filter(|value| !value.is_empty())?;
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "login-base".to_string());

    let exporter = match SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
    {
        Ok(exporter) => exporter,
        Err(err) => {
            eprintln!("failed to initialize OTLP tracing: {err}");
            return None;
        }
    };

    let resource = Resource::builder_empty()
        .with_attributes([KeyValue::new("service.name", service_name)])
        .build();

    Some(
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_batch_exporter(exporter)
            .build(),
    )
}
