use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use crate::app::login::{LoginError, LoginRequest, LoginUseCase};
use crate::http::dto::{
    HealthResponseDto, LoginErrorDto, LoginRequestDto, LoginResponseDto, LoginSuccessDto,
};

/// Shared Axum state for the login routes.
#[derive(Clone)]
pub struct AppState {
    /// Login use case injected at startup.
    pub service: Arc<dyn LoginUseCase + Send + Sync>,
}

/// Builds the HTTP router for the login API.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/login", post(login_handler))
        .with_state(state)
}

async fn health_handler() -> Json<HealthResponseDto> {
    Json(HealthResponseDto { status: "ok" })
}

async fn login_handler(
    State(state): State<AppState>,
    Json(body): Json<LoginRequestDto>,
) -> (StatusCode, Json<LoginResponseDto>) {
    let req = LoginRequest {
        username: body.username,
        password: body.password,
    };

    match state.service.login(req) {
        Ok(res) => (
            StatusCode::OK,
            Json(LoginResponseDto::Success(LoginSuccessDto {
                message: res.message,
                token: res.token,
            })),
        ),
        Err(err) => (
            map_login_error(&err),
            Json(LoginResponseDto::Error(LoginErrorDto {
                message: safe_error_message().to_string(),
            })),
        ),
    }
}

fn map_login_error(err: &LoginError) -> StatusCode {
    match err {
        LoginError::InvalidCredentials
        | LoginError::AccountLocked
        | LoginError::SelfExcluded => StatusCode::UNAUTHORIZED,
    }
}

fn safe_error_message() -> &'static str {
    "Authentication failed"
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hyper::body::to_bytes;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::app::login::{LoginRequest, LoginResponse, LoginResult, LoginUseCase};

    use super::{AppState, router};

    struct StubLoginUseCase;

    impl LoginUseCase for StubLoginUseCase {
        fn login(&self, req: LoginRequest) -> LoginResult {
            match req.username.as_str() {
                "demo" if req.password == "password" => Ok(LoginResponse {
                    message: "OK".to_string(),
                    token: "stub-jwt".to_string(),
                }),
                "locked" => Err(crate::app::login::LoginError::AccountLocked),
                "excluded" => Err(crate::app::login::LoginError::SelfExcluded),
                _ => Err(crate::app::login::LoginError::InvalidCredentials),
            }
        }
    }

    fn build_app() -> axum::Router {
        router(AppState {
            service: Arc::new(StubLoginUseCase),
        })
    }

    async fn post_login(username: &str, password: &str) -> (StatusCode, Value) {
        let app = build_app();
        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "username": username,
                    "password": password,
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body()).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();

        (status, json)
    }

    async fn get_health() -> (StatusCode, Value) {
        let app = build_app();
        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body()).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();

        (status, json)
    }

    #[tokio::test]
    async fn health_route_returns_ok() {
        let (status, body) = get_health().await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok" }));
    }

    #[tokio::test]
    async fn login_route_returns_ok_for_valid_credentials() {
        let (status, body) = post_login("demo", "password").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({ "status": "ok", "message": "OK", "token": "stub-jwt" })
        );
    }

    #[tokio::test]
    async fn login_route_returns_unauthorized_for_unknown_user() {
        let (status, body) = post_login("missing", "password").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Authentication failed" }));
    }

    #[tokio::test]
    async fn login_route_returns_unauthorized_for_wrong_password() {
        let (status, body) = post_login("demo", "wrong-password").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Authentication failed" }));
    }

    #[tokio::test]
    async fn login_route_returns_generic_failure_for_locked_account() {
        let (status, body) = post_login("locked", "password").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Authentication failed" }));
    }

    #[tokio::test]
    async fn login_route_returns_generic_failure_for_self_excluded_account() {
        let (status, body) = post_login("excluded", "password").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Authentication failed" }));
    }
}
