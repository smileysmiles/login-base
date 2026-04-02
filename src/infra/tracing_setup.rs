use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Initializes structured tracing logs once per process.
pub fn init_tracing() -> Option<SdkTracerProvider> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("login_base=info,tower_http=info"));

    match build_tracer_provider() {
        Some((provider, sampler_name, sampler_ratio)) => {
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
            tracing::info!(
                sampler = %sampler_name,
                sampler_ratio = ?sampler_ratio,
                "otlp trace export enabled"
            );
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

fn build_tracer_provider() -> Option<(SdkTracerProvider, String, Option<f64>)> {
    let endpoint =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok().filter(|value| !value.is_empty())?;
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "login-base".to_string());
    let (sampler, sampler_name, sampler_ratio) = resolve_sampler();

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

    Some((
        SdkTracerProvider::builder()
            .with_resource(resource)
            .with_sampler(sampler)
            .with_batch_exporter(exporter)
            .build(),
        sampler_name,
        sampler_ratio,
    ))
}

fn resolve_sampler() -> (Sampler, String, Option<f64>) {
    let configured = std::env::var("LOGIN_BASE_TRACE_SAMPLER")
        .or_else(|_| std::env::var("OTEL_TRACES_SAMPLER"))
        .unwrap_or_else(|_| "parentbased_traceidratio".to_string());

    let ratio = std::env::var("LOGIN_BASE_TRACE_SAMPLER_RATIO")
        .or_else(|_| std::env::var("OTEL_TRACES_SAMPLER_ARG"))
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.05);

    match configured.trim().to_ascii_lowercase().as_str() {
        "always_on" => (Sampler::AlwaysOn, "always_on".to_string(), None),
        "always_off" => (Sampler::AlwaysOff, "always_off".to_string(), None),
        "traceidratio" => (
            Sampler::TraceIdRatioBased(ratio),
            "traceidratio".to_string(),
            Some(ratio),
        ),
        "parentbased_always_on" => (
            Sampler::ParentBased(Box::new(Sampler::AlwaysOn)),
            "parentbased_always_on".to_string(),
            None,
        ),
        "parentbased_always_off" => (
            Sampler::ParentBased(Box::new(Sampler::AlwaysOff)),
            "parentbased_always_off".to_string(),
            None,
        ),
        "parentbased_traceidratio" => (
            Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio))),
            "parentbased_traceidratio".to_string(),
            Some(ratio),
        ),
        other => {
            eprintln!(
                "unknown sampler '{other}', using parentbased_traceidratio with ratio {ratio}"
            );
            (
                Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio))),
                "parentbased_traceidratio".to_string(),
                Some(ratio),
            )
        }
    }
}
