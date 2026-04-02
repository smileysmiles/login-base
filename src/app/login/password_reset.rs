use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::ports::AuthAccountRepository;
use crate::app::ports::{AuthBusinessEvent, Observability, PasswordResetFailureReason};

const RESET_TOKEN_TTL_SECONDS: u64 = 900;

#[derive(Debug)]
pub struct ForgotPasswordRequest {
    pub username: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ForgotPasswordResponse {
    pub message: String,
    pub reset_token: String,
}

#[derive(Debug)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResetPasswordResponse {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResetPasswordError {
    InvalidToken,
    PasswordReuseNotAllowed,
}

pub type ResetPasswordResult = Result<ResetPasswordResponse, ResetPasswordError>;

pub trait PasswordResetUseCase {
    fn request_reset(&self, req: ForgotPasswordRequest) -> ForgotPasswordResponse;
    fn reset_password(&self, req: ResetPasswordRequest) -> ResetPasswordResult;
}

/// Issues password reset tokens and applies password resets for matching accounts.
pub struct PasswordResetService<R: AuthAccountRepository> {
    auth_accounts: R,
    next_token_id: AtomicU64,
    observability: Arc<dyn Observability + Send + Sync>,
}

impl<R: AuthAccountRepository> PasswordResetService<R> {
    pub fn new(auth_accounts: R, observability: Arc<dyn Observability + Send + Sync>) -> Self {
        Self {
            auth_accounts,
            next_token_id: AtomicU64::new(1),
            observability,
        }
    }

    fn next_reset_token(&self) -> String {
        let id = self.next_token_id.fetch_add(1, Ordering::Relaxed);
        format!("reset-{}-{}", unix_timestamp(), id)
    }
}

impl<R: AuthAccountRepository> PasswordResetUseCase for PasswordResetService<R> {
    fn request_reset(&self, req: ForgotPasswordRequest) -> ForgotPasswordResponse {
        let reset_token = self.next_reset_token();
        let mut account_found = false;

        if let Some(mut account) = self.auth_accounts.get_by_username(&req.username) {
            account_found = true;
            account.issue_password_reset(
                reset_token.clone(),
                unix_timestamp() + RESET_TOKEN_TTL_SECONDS,
            );
            self.auth_accounts.save(account);
        }
        self.observability.emit(AuthBusinessEvent::PasswordResetRequested {
            username: req.username,
            account_found,
        });

        ForgotPasswordResponse {
            message: "If the account exists, reset instructions have been issued".to_string(),
            reset_token,
        }
    }

    fn reset_password(&self, req: ResetPasswordRequest) -> ResetPasswordResult {
        let Some(mut account) = self.auth_accounts.get_by_reset_token(&req.token) else {
            self.observability.emit(AuthBusinessEvent::PasswordResetFailed {
                reason: PasswordResetFailureReason::InvalidToken,
            });
            return Err(ResetPasswordError::InvalidToken);
        };

        if !account.can_reset_with(&req.token, unix_timestamp()) {
            self.observability.emit(AuthBusinessEvent::PasswordResetFailed {
                reason: PasswordResetFailureReason::InvalidToken,
            });
            return Err(ResetPasswordError::InvalidToken);
        }

        if account.password_hash == req.new_password {
            self.observability.emit(AuthBusinessEvent::PasswordResetFailed {
                reason: PasswordResetFailureReason::PasswordReuseNotAllowed,
            });
            return Err(ResetPasswordError::PasswordReuseNotAllowed);
        }

        let account_id = account.id;
        let username = account.username.clone();
        account.set_password(req.new_password);
        self.auth_accounts.save(account);
        self.observability.emit(AuthBusinessEvent::PasswordResetCompleted {
            account_id,
            username,
        });

        Ok(ResetPasswordResponse {
            message: "Password reset".to_string(),
        })
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};

    use crate::app::ports::AuthBusinessEvent;
    use crate::domain::auth_account::AuthAccount;

    use super::*;

    struct TestAuthAccountRepository {
        account: RefCell<Option<AuthAccount>>,
    }

    impl AuthAccountRepository for TestAuthAccountRepository {
        fn get_by_username(&self, username: &str) -> Option<AuthAccount> {
            self.account
                .borrow()
                .as_ref()
                .filter(|account| account.username == username)
                .cloned()
        }

        fn get_by_reset_token(&self, token: &str) -> Option<AuthAccount> {
            self.account
                .borrow()
                .as_ref()
                .filter(|account| account.password_reset_token.as_deref() == Some(token))
                .cloned()
        }

        fn save(&self, account: AuthAccount) {
            *self.account.borrow_mut() = Some(account);
        }
    }

    #[derive(Default)]
    struct TestObservability {
        events: Mutex<Vec<AuthBusinessEvent>>,
    }

    impl Observability for TestObservability {
        fn emit(&self, event: AuthBusinessEvent) {
            self.events
                .lock()
                .expect("observability mutex should be available")
                .push(event);
        }
    }

    fn test_observability() -> Arc<TestObservability> {
        Arc::new(TestObservability::default())
    }

    #[test]
    fn forgot_password_issues_a_reset_token_for_existing_account() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", false))),
        };
        let service = PasswordResetService::new(repo, observability.clone());

        let result = service.request_reset(ForgotPasswordRequest {
            username: "demo".to_string(),
        });

        assert_eq!(
            result.message,
            "If the account exists, reset instructions have been issued"
        );
        assert!(result.reset_token.starts_with("reset-"));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordResetRequested {
                username: "demo".to_string(),
                account_found: true,
            }]
        );
    }

    #[test]
    fn forgot_password_still_returns_generic_message_for_unknown_account() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(None),
        };
        let service = PasswordResetService::new(repo, observability.clone());

        let result = service.request_reset(ForgotPasswordRequest {
            username: "missing".to_string(),
        });

        assert!(result.reset_token.starts_with("reset-"));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordResetRequested {
                username: "missing".to_string(),
                account_found: false,
            }]
        );
    }

    #[test]
    fn reset_password_succeeds_for_valid_reset_token() {
        let mut account = AuthAccount::new(1, "demo", "password", true);
        account.failed_login_attempts = 3;
        account.issue_password_reset("valid-token", unix_timestamp() + 300);
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(account)),
        };
        let service = PasswordResetService::new(repo, observability.clone());

        let result = service.reset_password(ResetPasswordRequest {
            token: "valid-token".to_string(),
            new_password: "new-password".to_string(),
        });

        assert_eq!(
            result,
            Ok(ResetPasswordResponse {
                message: "Password reset".to_string(),
            })
        );
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordResetCompleted {
                account_id: 1,
                username: "demo".to_string(),
            }]
        );
    }

    #[test]
    fn reset_password_fails_for_invalid_reset_token() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", false))),
        };
        let service = PasswordResetService::new(repo, observability.clone());

        let result = service.reset_password(ResetPasswordRequest {
            token: "missing-token".to_string(),
            new_password: "new-password".to_string(),
        });

        assert_eq!(result, Err(ResetPasswordError::InvalidToken));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordResetFailed {
                reason: PasswordResetFailureReason::InvalidToken,
            }]
        );
    }

    #[test]
    fn reset_password_rejects_reusing_the_existing_password() {
        let mut account = AuthAccount::new(1, "demo", "password", false);
        account.issue_password_reset("valid-token", unix_timestamp() + 300);
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(account)),
        };
        let service = PasswordResetService::new(repo, observability.clone());

        let result = service.reset_password(ResetPasswordRequest {
            token: "valid-token".to_string(),
            new_password: "password".to_string(),
        });

        assert_eq!(result, Err(ResetPasswordError::PasswordReuseNotAllowed));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordResetFailed {
                reason: PasswordResetFailureReason::PasswordReuseNotAllowed,
            }]
        );
    }
}
