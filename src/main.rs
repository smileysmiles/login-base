mod domain;
mod app;
mod infra;
mod http;

use std::net::SocketAddr;
use std::sync::Arc;

use app::login::LoginService;
use axum::{Server};
use http::routes::{router, AppState};
use infra::in_memory_player_repo::InMemoryPlayerRepository;
use infra::mock_compliance::MockComplianceService;
use tower_http::cors::CorsLayer;
use axum::http::{Method, header, HeaderValue};


#[tokio::main]
async fn main() {
    // Build infra
    let repo = InMemoryPlayerRepository::new_with_demo_user();
    let compliance = MockComplianceService;

    // Build application service
    let service = LoginService::new(repo, compliance);

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
