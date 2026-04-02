mod domain;
mod app;
mod infra;
mod http;

use std::net::SocketAddr;
use std::sync::Arc;

use app::login::{AuthenticationService, ChangePasswordService, LoginService, PasswordResetService};
use app::ports::Observability;
use axum::http::{HeaderValue, Method, header};
use axum::Server;
use http::routes::{router, AppState};
use infra::in_memory_auth_account_repo::InMemoryAuthAccountRepository;
use infra::in_memory_session_store::InMemorySessionStore;
use infra::jwt_session_manager::JwtSessionManager;
use infra::jwt_token_issuer::JwtTokenIssuer;
use infra::mock_compliance::MockComplianceService;
use infra::mock_observability::MockObservability;
use infra::noop_observability::NoopObservability;
use infra::telemetry_observability::TelemetryObservability;
use infra::tracing_setup::init_tracing;
use tower_http::classify::ServerErrorsFailureClass;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::Span;


#[tokio::main]
async fn main() {
    let tracer_provider = init_tracing();

    let jwt_secret = "local-dev-secret";
    let observability_mode =
        std::env::var("LOGIN_BASE_OBSERVABILITY").unwrap_or_else(|_| "telemetry".to_string());
    let http_trace_enabled = env_flag("LOGIN_BASE_HTTP_TRACE_ENABLED", true);

    // Build infra
    let repo = InMemoryAuthAccountRepository::new_with_demo_users(100);
    let compliance = MockComplianceService;
    let observability: Arc<dyn Observability + Send + Sync> = match observability_mode.as_str() {
        "none" => Arc::new(NoopObservability),
        "mock" => Arc::new(MockObservability),
        _ => Arc::new(TelemetryObservability::default()),
    };
    let session_store = InMemorySessionStore::new();
    let token_issuer = JwtTokenIssuer::new(jwt_secret, 3600, session_store.clone());
    let token_sessions = JwtSessionManager::new(jwt_secret, session_store);

    // Build application services
    let authenticator = AuthenticationService::new(repo.clone(), compliance, observability.clone());
    let service = LoginService::new(authenticator, token_issuer);
    let change_password_service = ChangePasswordService::new(repo.clone(), observability.clone());
    let password_reset_service = PasswordResetService::new(repo, observability.clone());

    // Shared state for Axum
    let state = AppState {
        login_service: Arc::new(service),
        change_password_service: Arc::new(change_password_service),
        password_reset_service: Arc::new(password_reset_service),
        token_sessions: Arc::new(token_sessions),
        observability,
    };

    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<HeaderValue>()
                .expect("valid origin"),
        )
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    let app = router(state).layer(cors);
    let app = if http_trace_enabled {
        app.layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
            let request_id = request
                .headers()
                .get("x-request-id")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("missing");

            tracing::info_span!(
                "http.request",
                method = %request.method(),
                path = %request.uri().path(),
                request_id = %request_id
            )
        }).on_response(
            |response: &axum::http::Response<_>, latency: std::time::Duration, _span: &Span| {
                tracing::info!(
                    status = response.status().as_u16(),
                    latency_ms = latency.as_millis() as u64,
                    "request completed"
                );
            },
        ).on_failure(
            |error: ServerErrorsFailureClass, latency: std::time::Duration, _span: &Span| {
                tracing::warn!(
                    error = ?error,
                    latency_ms = latency.as_millis() as u64,
                    "request failed"
                );
            },
        ))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
    } else {
        app
    };

    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    tracing::info!(
        address = %addr,
        observability_mode = %observability_mode,
        http_trace_enabled,
        "listening"
    );

    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    if let Some(provider) = tracer_provider {
        let _ = provider.shutdown();
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "off" | "no" => false,
            "1" | "true" | "on" | "yes" => true,
            _ => default,
        },
        Err(_) => default,
    }
}
