mod domain;
mod app;
mod infra;
mod http;

use std::net::SocketAddr;
use std::sync::Arc;

use app::login::{AuthenticationService, LoginService};
use axum::http::{HeaderValue, Method, header};
use axum::Server;
use http::routes::{router, AppState};
use infra::in_memory_auth_account_repo::InMemoryAuthAccountRepository;
use infra::jwt_token_issuer::JwtTokenIssuer;
use infra::mock_compliance::MockComplianceService;
use tower_http::cors::CorsLayer;


#[tokio::main]
async fn main() {
    // Build infra
    let repo = InMemoryAuthAccountRepository::new_with_demo_user();
    let compliance = MockComplianceService;
    let token_issuer = JwtTokenIssuer::new("local-dev-secret", 3600);

    // Build application services
    let authenticator = AuthenticationService::new(repo, compliance);
    let service = LoginService::new(authenticator, token_issuer);

    // Shared state for Axum
    let state = AppState {
        service: Arc::new(service),
    };

    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:5173"
                .parse::<HeaderValue>()
                .expect("valid origin"),
        )
        .allow_methods([Method::POST])
        .allow_headers([header::CONTENT_TYPE]);

    let app = router(state).layer(cors);

    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    println!("Listening on http://{}", addr);

    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
