use std::sync::atomic::{AtomicU64, Ordering};

use tracing::{info, info_span, warn};

use crate::app::ports::{
    AuthBusinessEvent, LoginFailureReason, LogoutFailureReason, MeLookupFailureReason,
    Observability, PasswordChangeFailureReason, PasswordResetFailureReason,
};

/// Thread-safe counters for core auth business events.
#[derive(Default)]
struct EventCounters {
    login_succeeded: AtomicU64,
    login_failed: AtomicU64,
    account_locked: AtomicU64,
    password_changed: AtomicU64,
    password_change_failed: AtomicU64,
    password_reset_requested: AtomicU64,
    password_reset_completed: AtomicU64,
    password_reset_failed: AtomicU64,
    logout_succeeded: AtomicU64,
    logout_failed: AtomicU64,
    me_lookup_succeeded: AtomicU64,
    me_lookup_failed: AtomicU64,
}

/// Observability adapter that emits structured logs and counters for auth events.
#[derive(Default)]
pub struct TelemetryObservability {
    counters: EventCounters,
}

impl TelemetryObservability {
    fn inc(counter: &AtomicU64) -> u64 {
        counter.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl Observability for TelemetryObservability {
    fn emit(&self, event: AuthBusinessEvent) {
        match event {
            AuthBusinessEvent::LoginSucceeded {
                account_id,
                username,
            } => {
                let count = Self::inc(&self.counters.login_succeeded);
                let _span = info_span!("auth_business_event", event = "login_succeeded").entered();
                info!(
                    account_id,
                    username = %username,
                    metric_login_succeeded = count,
                    "login succeeded"
                );
            }
            AuthBusinessEvent::LoginFailed { username, reason } => {
                let count = Self::inc(&self.counters.login_failed);
                let _span = info_span!("auth_business_event", event = "login_failed").entered();
                warn!(
                    username = %username,
                    reason = %login_failure_reason(reason),
                    metric_login_failed = count,
                    "login failed"
                );
            }
            AuthBusinessEvent::AccountLocked {
                account_id,
                username,
                failed_attempts,
            } => {
                let count = Self::inc(&self.counters.account_locked);
                let _span = info_span!("auth_business_event", event = "account_locked").entered();
                warn!(
                    account_id,
                    username = %username,
                    failed_attempts,
                    metric_account_locked = count,
                    "account locked"
                );
            }
            AuthBusinessEvent::PasswordChanged {
                account_id,
                username,
            } => {
                let count = Self::inc(&self.counters.password_changed);
                let _span = info_span!("auth_business_event", event = "password_changed").entered();
                info!(
                    account_id,
                    username = %username,
                    metric_password_changed = count,
                    "password changed"
                );
            }
            AuthBusinessEvent::PasswordChangeFailed { username, reason } => {
                let count = Self::inc(&self.counters.password_change_failed);
                let _span =
                    info_span!("auth_business_event", event = "password_change_failed").entered();
                warn!(
                    username = %username,
                    reason = %password_change_failure_reason(reason),
                    metric_password_change_failed = count,
                    "password change failed"
                );
            }
            AuthBusinessEvent::PasswordResetRequested {
                username,
                account_found,
            } => {
                let count = Self::inc(&self.counters.password_reset_requested);
                let _span =
                    info_span!("auth_business_event", event = "password_reset_requested").entered();
                info!(
                    username = %username,
                    account_found,
                    metric_password_reset_requested = count,
                    "password reset requested"
                );
            }
            AuthBusinessEvent::PasswordResetCompleted {
                account_id,
                username,
            } => {
                let count = Self::inc(&self.counters.password_reset_completed);
                let _span =
                    info_span!("auth_business_event", event = "password_reset_completed").entered();
                info!(
                    account_id,
                    username = %username,
                    metric_password_reset_completed = count,
                    "password reset completed"
                );
            }
            AuthBusinessEvent::PasswordResetFailed { reason } => {
                let count = Self::inc(&self.counters.password_reset_failed);
                let _span =
                    info_span!("auth_business_event", event = "password_reset_failed").entered();
                warn!(
                    reason = %password_reset_failure_reason(reason),
                    metric_password_reset_failed = count,
                    "password reset failed"
                );
            }
            AuthBusinessEvent::LogoutSucceeded {
                account_id,
                username,
            } => {
                let count = Self::inc(&self.counters.logout_succeeded);
                let _span = info_span!("auth_business_event", event = "logout_succeeded").entered();
                info!(
                    account_id = ?account_id,
                    username = ?username,
                    metric_logout_succeeded = count,
                    "logout succeeded"
                );
            }
            AuthBusinessEvent::LogoutFailed { reason } => {
                let count = Self::inc(&self.counters.logout_failed);
                let _span = info_span!("auth_business_event", event = "logout_failed").entered();
                warn!(
                    reason = %logout_failure_reason(reason),
                    metric_logout_failed = count,
                    "logout failed"
                );
            }
            AuthBusinessEvent::MeLookupSucceeded {
                account_id,
                username,
            } => {
                let count = Self::inc(&self.counters.me_lookup_succeeded);
                let _span =
                    info_span!("auth_business_event", event = "me_lookup_succeeded").entered();
                info!(
                    account_id,
                    username = %username,
                    metric_me_lookup_succeeded = count,
                    "me lookup succeeded"
                );
            }
            AuthBusinessEvent::MeLookupFailed { reason } => {
                let count = Self::inc(&self.counters.me_lookup_failed);
                let _span = info_span!("auth_business_event", event = "me_lookup_failed").entered();
                warn!(
                    reason = %me_lookup_failure_reason(reason),
                    metric_me_lookup_failed = count,
                    "me lookup failed"
                );
            }
        }
    }
}

fn login_failure_reason(reason: LoginFailureReason) -> &'static str {
    match reason {
        LoginFailureReason::UnknownUsername => "unknown_username",
        LoginFailureReason::WrongPassword => "wrong_password",
        LoginFailureReason::AccountLocked => "account_locked",
        LoginFailureReason::SelfExcluded => "self_excluded",
    }
}

fn password_change_failure_reason(reason: PasswordChangeFailureReason) -> &'static str {
    match reason {
        PasswordChangeFailureReason::UnknownUsername => "unknown_username",
        PasswordChangeFailureReason::InvalidCurrentPassword => "invalid_current_password",
        PasswordChangeFailureReason::AccountLocked => "account_locked",
        PasswordChangeFailureReason::PasswordReuseNotAllowed => "password_reuse_not_allowed",
    }
}

fn password_reset_failure_reason(reason: PasswordResetFailureReason) -> &'static str {
    match reason {
        PasswordResetFailureReason::InvalidToken => "invalid_token",
        PasswordResetFailureReason::PasswordReuseNotAllowed => "password_reuse_not_allowed",
    }
}

fn logout_failure_reason(reason: LogoutFailureReason) -> &'static str {
    match reason {
        LogoutFailureReason::MissingBearerToken => "missing_bearer_token",
        LogoutFailureReason::InvalidOrRevokedToken => "invalid_or_revoked_token",
    }
}

fn me_lookup_failure_reason(reason: MeLookupFailureReason) -> &'static str {
    match reason {
        MeLookupFailureReason::MissingBearerToken => "missing_bearer_token",
        MeLookupFailureReason::InvalidOrRevokedToken => "invalid_or_revoked_token",
    }
}
