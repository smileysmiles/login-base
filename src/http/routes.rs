use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};

use crate::app::login::{LoginRequest, LoginService};
use crate::http::dto::{LoginRequestDto, LoginResponseDto};
use crate::infra::in_memory_player_repo::InMemoryPlayerRepository;
use crate::infra::mock_compliance::MockComplianceService;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<LoginService<InMemoryPlayerRepository, MockComplianceService>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/login", post(login_handler))
        .with_state(state)
}

async fn login_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginRequestDto>,
) -> (StatusCode, Json<LoginResponseDto>) {
    let req = LoginRequest {
        username: body.username,
        password: body.password,
    };

    let res = state.service.login(req);

    let status = if res.success {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    };

    (
        status,
        Json(LoginResponseDto {
            success: res.success,
            message: res.message,
        }),
    )
}
