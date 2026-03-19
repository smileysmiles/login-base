use super::errors::LoginError;

/// Successful internal login result.
#[derive(Debug, PartialEq, Eq)]
pub struct LoginResponse {
    pub message: String,
}

/// Internal login outcome used by the application layer.
pub type LoginResult = Result<LoginResponse, LoginError>;
