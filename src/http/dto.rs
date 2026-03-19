use serde::{Deserialize, Serialize};

/// JSON payload accepted by `POST /login`.
#[derive(Debug, Deserialize)]
pub struct LoginRequestDto {
    pub username: String,
    pub password: String,
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
#[derive(Debug, Serialize)]
pub struct LoginSuccessDto {
    pub message: String,
    pub token: String,
}

/// Generic error payload returned to the client.
#[derive(Debug, Serialize)]
pub struct LoginErrorDto {
    pub message: String,
}

/// JSON response returned by `GET /health`.
#[derive(Debug, Serialize)]
pub struct HealthResponseDto {
    pub status: &'static str,
}
