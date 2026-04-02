use serde::{Deserialize, Serialize};

/// JSON payload accepted by `POST /login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequestDto {
    pub username: String,
    pub password: String,
}

/// JSON payload accepted by `POST /change-password`.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequestDto {
    pub username: String,
    pub current_password: String,
    pub new_password: String,
}

/// JSON payload accepted by `POST /forgot-password`.
#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequestDto {
    pub username: String,
}

/// JSON payload accepted by `POST /reset-password`.
#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequestDto {
    pub token: String,
    pub new_password: String,
}

/// JSON response returned by `POST /login`.
#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum LoginResponseDto {
    /// Successful authentication response.
    #[serde(rename = "ok")]
    Success(LoginSuccessDto),
    /// Generic authentication failure response.
    #[serde(rename = "error")]
    Error(LoginErrorDto),
}

/// Success payload returned to the client.
/// This stays intentionally narrow so auth does not become a player-profile API.
#[derive(Debug, Serialize)]
pub struct LoginSuccessDto {
    pub account_id: u64,
    pub username: String,
    pub message: String,
    pub token: String,
}

/// Generic error payload returned to the client.
#[derive(Debug, Serialize)]
pub struct LoginErrorDto {
    pub message: String,
}

/// JSON response returned by successful password management routes.
#[derive(Debug, Serialize)]
pub struct PasswordActionResponseDto {
    pub status: &'static str,
    pub message: String,
}

/// JSON response returned by `POST /forgot-password`.
#[derive(Debug, Serialize)]
pub struct ForgotPasswordResponseDto {
    pub status: &'static str,
    pub message: String,
    pub reset_token: String,
}

/// JSON response returned by `POST /logout`.
#[derive(Debug, Serialize)]
pub struct LogoutResponseDto {
    pub status: &'static str,
    pub message: &'static str,
}

/// JSON response returned by `GET /me`.
/// This represents the authenticated subject only, not richer player profile data.
#[derive(Debug, Serialize)]
pub struct MeResponseDto {
    pub account_id: u64,
    pub username: String,
}

/// JSON response returned by `GET /health`.
#[derive(Debug, Serialize)]
pub struct HealthResponseDto {
    pub status: &'static str,
}
