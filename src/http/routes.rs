use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, header},
    routing::{get, post},
    Json, Router,
};

use crate::app::login::{
    ChangePasswordError, ChangePasswordRequest, ChangePasswordUseCase, ForgotPasswordRequest,
    LoginError, LoginRequest, LoginUseCase, PasswordResetUseCase, ResetPasswordError,
    ResetPasswordRequest,
};
use crate::app::ports::{
    AuthBusinessEvent, LogoutFailureReason, MeLookupFailureReason, Observability,
    TokenSessionManager,
};
use crate::http::dto::{
    ChangePasswordRequestDto, ForgotPasswordRequestDto, ForgotPasswordResponseDto,
    HealthResponseDto, LoginErrorDto, LoginRequestDto, LoginResponseDto, LoginSuccessDto,
    LogoutResponseDto, MeResponseDto, PasswordActionResponseDto, ResetPasswordRequestDto,
};

/// Shared Axum state for the login routes.
#[derive(Clone)]
pub struct AppState {
    /// Login use case injected at startup.
    pub login_service: Arc<dyn LoginUseCase + Send + Sync>,
    /// Change-password use case injected at startup.
    pub change_password_service: Arc<dyn ChangePasswordUseCase + Send + Sync>,
    /// Forgot/reset password use case injected at startup.
    pub password_reset_service: Arc<dyn PasswordResetUseCase + Send + Sync>,
    /// Token validator and revocation boundary.
    pub token_sessions: Arc<dyn TokenSessionManager + Send + Sync>,
    /// Business/security observability boundary.
    pub observability: Arc<dyn Observability + Send + Sync>,
}

/// Builds the HTTP router for the login API.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/login", post(login_handler))
        .route("/change-password", post(change_password_handler))
        .route("/forgot-password", post(forgot_password_handler))
        .route("/reset-password", post(reset_password_handler))
        .route("/me", get(me_handler))
        .route("/logout", post(logout_handler))
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

    match state.login_service.login(req) {
        Ok(res) => (
            StatusCode::OK,
            Json(LoginResponseDto::Success(LoginSuccessDto {
                account_id: res.account_id,
                username: res.username,
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

async fn change_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ChangePasswordRequestDto>,
) -> (StatusCode, Json<PasswordActionResponseDto>) {
    let req = ChangePasswordRequest {
        username: body.username,
        current_password: body.current_password,
        new_password: body.new_password,
    };

    match state.change_password_service.change_password(req) {
        Ok(res) => (
            StatusCode::OK,
            Json(PasswordActionResponseDto {
                status: "ok",
                message: res.message,
            }),
        ),
        Err(err) => (
            map_change_password_error(&err),
            Json(PasswordActionResponseDto {
                status: "error",
                message: safe_password_action_message().to_string(),
            }),
        ),
    }
}

async fn forgot_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequestDto>,
) -> (StatusCode, Json<ForgotPasswordResponseDto>) {
    let res = state.password_reset_service.request_reset(ForgotPasswordRequest {
        username: body.username,
    });

    (
        StatusCode::OK,
        Json(ForgotPasswordResponseDto {
            status: "ok",
            message: res.message,
            reset_token: res.reset_token,
        }),
    )
}

async fn reset_password_handler(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequestDto>,
) -> (StatusCode, Json<PasswordActionResponseDto>) {
    let req = ResetPasswordRequest {
        token: body.token,
        new_password: body.new_password,
    };

    match state.password_reset_service.reset_password(req) {
        Ok(res) => (
            StatusCode::OK,
            Json(PasswordActionResponseDto {
                status: "ok",
                message: res.message,
            }),
        ),
        Err(err) => (
            map_reset_password_error(&err),
            Json(PasswordActionResponseDto {
                status: "error",
                message: safe_password_action_message().to_string(),
            }),
        ),
    }
}

async fn logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<LogoutResponseDto>) {
    let Some(token) = bearer_token(&headers) else {
        state.observability.emit(AuthBusinessEvent::LogoutFailed {
            reason: LogoutFailureReason::MissingBearerToken,
        });
        return unauthorized_logout_response();
    };

    if !state.token_sessions.is_active(token) {
        state.observability.emit(AuthBusinessEvent::LogoutFailed {
            reason: LogoutFailureReason::InvalidOrRevokedToken,
        });
        return unauthorized_logout_response();
    }

    let user = state.token_sessions.current_user(token);
    state.token_sessions.revoke(token);
    state.observability.emit(AuthBusinessEvent::LogoutSucceeded {
        account_id: user.as_ref().map(|u| u.account_id),
        username: user.map(|u| u.username),
    });

    (
        StatusCode::OK,
        Json(LogoutResponseDto {
            status: "ok",
            message: "Logged out",
        }),
    )
}

async fn me_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MeResponseDto>, StatusCode> {
    let Some(token) = bearer_token(&headers) else {
        state.observability.emit(AuthBusinessEvent::MeLookupFailed {
            reason: MeLookupFailureReason::MissingBearerToken,
        });
        return Err(StatusCode::UNAUTHORIZED);
    };

    let Some(user) = state.token_sessions.current_user(token) else {
        state.observability.emit(AuthBusinessEvent::MeLookupFailed {
            reason: MeLookupFailureReason::InvalidOrRevokedToken,
        });
        return Err(StatusCode::UNAUTHORIZED);
    };
    state.observability.emit(AuthBusinessEvent::MeLookupSucceeded {
        account_id: user.account_id,
        username: user.username.clone(),
    });

    Ok(Json(MeResponseDto {
        account_id: user.account_id,
        username: user.username,
    }))
}

fn map_login_error(err: &LoginError) -> StatusCode {
    match err {
        LoginError::InvalidCredentials
        | LoginError::AccountLocked
        | LoginError::SelfExcluded => StatusCode::UNAUTHORIZED,
    }
}

fn map_change_password_error(err: &ChangePasswordError) -> StatusCode {
    match err {
        ChangePasswordError::InvalidCredentials | ChangePasswordError::AccountLocked => {
            StatusCode::UNAUTHORIZED
        }
        ChangePasswordError::PasswordReuseNotAllowed => StatusCode::BAD_REQUEST,
    }
}

fn map_reset_password_error(err: &ResetPasswordError) -> StatusCode {
    match err {
        ResetPasswordError::InvalidToken | ResetPasswordError::PasswordReuseNotAllowed => {
            StatusCode::BAD_REQUEST
        }
    }
}

fn safe_error_message() -> &'static str {
    "Authentication failed"
}

fn safe_password_action_message() -> &'static str {
    "Password action failed"
}

fn unauthorized_logout_response() -> (StatusCode, Json<LogoutResponseDto>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(LogoutResponseDto {
            status: "error",
            message: "Unauthorized",
        }),
    )
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use hyper::body::to_bytes;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::app::login::{
        ChangePasswordError, ChangePasswordRequest, ChangePasswordResponse, ChangePasswordUseCase,
        ForgotPasswordRequest, ForgotPasswordResponse, LoginRequest, LoginResponse, LoginResult,
        LoginUseCase, PasswordResetUseCase, ResetPasswordError, ResetPasswordRequest,
        ResetPasswordResponse,
    };
    use crate::app::ports::{AuthBusinessEvent, Observability, TokenSessionManager};

    use super::{AppState, router};

    // The HTTP tests stub the use case and token session boundary so they only
    // verify route wiring, response shapes, and status mapping.
    struct StubLoginUseCase;

    impl LoginUseCase for StubLoginUseCase {
        fn login(&self, req: LoginRequest) -> LoginResult {
            match req.username.as_str() {
                "demo" if req.password == "password" => Ok(LoginResponse {
                    account_id: 1,
                    username: "demo".to_string(),
                    message: "OK".to_string(),
                    token: "stub-jwt".to_string(),
                }),
                "locked" => Err(crate::app::login::LoginError::AccountLocked),
                "excluded" => Err(crate::app::login::LoginError::SelfExcluded),
                _ => Err(crate::app::login::LoginError::InvalidCredentials),
            }
        }
    }

    struct StubChangePasswordUseCase;

    impl ChangePasswordUseCase for StubChangePasswordUseCase {
        fn change_password(
            &self,
            req: ChangePasswordRequest,
        ) -> Result<ChangePasswordResponse, ChangePasswordError> {
            match req.username.as_str() {
                "demo" if req.current_password == "password" && req.new_password != "password" => {
                    Ok(ChangePasswordResponse {
                        message: "Password changed".to_string(),
                    })
                }
                "locked" => Err(ChangePasswordError::AccountLocked),
                "demo" if req.new_password == "password" => {
                    Err(ChangePasswordError::PasswordReuseNotAllowed)
                }
                _ => Err(ChangePasswordError::InvalidCredentials),
            }
        }
    }

    struct StubPasswordResetUseCase;

    impl PasswordResetUseCase for StubPasswordResetUseCase {
        fn request_reset(&self, req: ForgotPasswordRequest) -> ForgotPasswordResponse {
            ForgotPasswordResponse {
                message: "If the account exists, reset instructions have been issued".to_string(),
                reset_token: format!("reset-token-for-{}", req.username),
            }
        }

        fn reset_password(
            &self,
            req: ResetPasswordRequest,
        ) -> Result<ResetPasswordResponse, ResetPasswordError> {
            match req.token.as_str() {
                "valid-reset-token" if req.new_password != "password" => Ok(ResetPasswordResponse {
                    message: "Password reset".to_string(),
                }),
                "valid-reset-token" => Err(ResetPasswordError::PasswordReuseNotAllowed),
                _ => Err(ResetPasswordError::InvalidToken),
            }
        }
    }

    // Session checks are reduced to a single known active token for boundary tests.
    struct StubTokenSessionManager;

    impl TokenSessionManager for StubTokenSessionManager {
        fn is_active(&self, token: &str) -> bool {
            token == "active-token"
        }

        fn current_user(&self, token: &str) -> Option<crate::app::ports::SessionUser> {
            (token == "active-token").then(|| crate::app::ports::SessionUser {
                account_id: 1,
                username: "demo".to_string(),
            })
        }

        fn revoke(&self, _token: &str) {}
    }

    #[derive(Default)]
    struct StubObservability {
        events: Mutex<Vec<AuthBusinessEvent>>,
    }

    impl Observability for StubObservability {
        fn emit(&self, event: AuthBusinessEvent) {
            self.events
                .lock()
                .expect("observability mutex should be available")
                .push(event);
        }
    }

    // Build the router once per request helper with deterministic doubles.
    fn build_app() -> axum::Router {
        router(AppState {
            login_service: Arc::new(StubLoginUseCase),
            change_password_service: Arc::new(StubChangePasswordUseCase),
            password_reset_service: Arc::new(StubPasswordResetUseCase),
            token_sessions: Arc::new(StubTokenSessionManager),
            observability: Arc::new(StubObservability::default()),
        })
    }

    // Login helper posts the expected request DTO and parses the JSON response body.
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

    async fn post_change_password(
        username: &str,
        current_password: &str,
        new_password: &str,
    ) -> (StatusCode, Value) {
        let app = build_app();
        let request = Request::builder()
            .method("POST")
            .uri("/change-password")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "username": username,
                    "current_password": current_password,
                    "new_password": new_password,
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

    async fn post_forgot_password(username: &str) -> (StatusCode, Value) {
        let app = build_app();
        let request = Request::builder()
            .method("POST")
            .uri("/forgot-password")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "username": username }).to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body()).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();

        (status, json)
    }

    async fn post_reset_password(token: &str, new_password: &str) -> (StatusCode, Value) {
        let app = build_app();
        let request = Request::builder()
            .method("POST")
            .uri("/reset-password")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "token": token,
                    "new_password": new_password,
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

    // Health is intentionally tiny; the helper mirrors the route shape.
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

    // Logout exercises bearer-token handling and revocation response mapping.
    async fn post_logout(token: Option<&str>) -> (StatusCode, Value) {
        let app = build_app();
        let mut request = Request::builder().method("POST").uri("/logout");

        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let response = app.oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body()).await.unwrap();
        let json = serde_json::from_slice(&body).unwrap();

        (status, json)
    }

    // Me uses the same bearer path, but unauthorized responses are intentionally empty.
    async fn get_me(token: Option<&str>) -> (StatusCode, Value) {
        let app = build_app();
        let mut request = Request::builder().method("GET").uri("/me");

        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let response = app.oneshot(request.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body()).await.unwrap();
        let json = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        };

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
            json!({
                "status": "ok",
                "account_id": 1,
                "username": "demo",
                "message": "OK",
                "token": "stub-jwt"
            })
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

    #[tokio::test]
    async fn change_password_route_returns_ok_for_valid_credentials() {
        let (status, body) = post_change_password("demo", "password", "new-password").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok", "message": "Password changed" }));
    }

    #[tokio::test]
    async fn change_password_route_returns_unauthorized_for_wrong_current_password() {
        let (status, body) = post_change_password("demo", "wrong-password", "new-password").await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Password action failed" }));
    }

    #[tokio::test]
    async fn forgot_password_route_returns_reset_token() {
        let (status, body) = post_forgot_password("demo").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({
                "status": "ok",
                "message": "If the account exists, reset instructions have been issued",
                "reset_token": "reset-token-for-demo"
            })
        );
    }

    #[tokio::test]
    async fn reset_password_route_returns_ok_for_valid_token() {
        let (status, body) = post_reset_password("valid-reset-token", "new-password").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok", "message": "Password reset" }));
    }

    #[tokio::test]
    async fn reset_password_route_returns_bad_request_for_invalid_token() {
        let (status, body) = post_reset_password("missing-token", "new-password").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, json!({ "status": "error", "message": "Password action failed" }));
    }

    #[tokio::test]
    async fn logout_route_returns_ok_for_active_token() {
        let (status, body) = post_logout(Some("active-token")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok", "message": "Logged out" }));
    }

    #[tokio::test]
    async fn logout_route_returns_unauthorized_without_bearer_token() {
        let (status, body) = post_logout(None).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Unauthorized" }));
    }

    #[tokio::test]
    async fn logout_route_returns_unauthorized_for_invalid_token() {
        let (status, body) = post_logout(Some("inactive-token")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, json!({ "status": "error", "message": "Unauthorized" }));
    }

    #[tokio::test]
    async fn me_route_returns_current_user_for_active_token() {
        let (status, body) = get_me(Some("active-token")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "account_id": 1, "username": "demo" }));
    }

    #[tokio::test]
    async fn me_route_returns_unauthorized_without_bearer_token() {
        let (status, body) = get_me(None).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, Value::Null);
    }

    #[tokio::test]
    async fn me_route_returns_unauthorized_for_invalid_token() {
        let (status, body) = get_me(Some("inactive-token")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body, Value::Null);
    }
}
