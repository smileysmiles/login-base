/// Internal reasons used for security-oriented login failure telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFailureReason {
    UnknownUsername,
    WrongPassword,
    AccountLocked,
    SelfExcluded,
}

/// Internal reasons used for change-password failure telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordChangeFailureReason {
    UnknownUsername,
    InvalidCurrentPassword,
    AccountLocked,
    PasswordReuseNotAllowed,
}

/// Internal reasons used for reset-password failure telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordResetFailureReason {
    InvalidToken,
    PasswordReuseNotAllowed,
}

/// Internal reasons used for logout failure telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutFailureReason {
    MissingBearerToken,
    InvalidOrRevokedToken,
}

/// Internal reasons used for authenticated-subject lookup failure telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeLookupFailureReason {
    MissingBearerToken,
    InvalidOrRevokedToken,
}

/// Business/security events emitted by the auth bounded context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthBusinessEvent {
    LoginSucceeded { account_id: u64, username: String },
    LoginFailed {
        username: String,
        reason: LoginFailureReason,
    },
    AccountLocked {
        account_id: u64,
        username: String,
        failed_attempts: u32,
    },
    PasswordChanged { account_id: u64, username: String },
    PasswordChangeFailed {
        username: String,
        reason: PasswordChangeFailureReason,
    },
    PasswordResetRequested {
        username: String,
        account_found: bool,
    },
    PasswordResetCompleted { account_id: u64, username: String },
    PasswordResetFailed { reason: PasswordResetFailureReason },
    LogoutSucceeded {
        account_id: Option<u64>,
        username: Option<String>,
    },
    LogoutFailed { reason: LogoutFailureReason },
    MeLookupSucceeded { account_id: u64, username: String },
    MeLookupFailed { reason: MeLookupFailureReason },
}

/// Port for emitting auth-domain observability signals to external systems.
pub trait Observability {
    fn emit(&self, event: AuthBusinessEvent);
}
