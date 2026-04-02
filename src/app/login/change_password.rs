use std::sync::Arc;

use crate::app::ports::AuthAccountRepository;
use crate::app::ports::{
    AuthBusinessEvent, Observability, PasswordChangeFailureReason,
};

#[derive(Debug)]
pub struct ChangePasswordRequest {
    pub username: String,
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ChangePasswordResponse {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangePasswordError {
    InvalidCredentials,
    AccountLocked,
    PasswordReuseNotAllowed,
}

pub type ChangePasswordResult = Result<ChangePasswordResponse, ChangePasswordError>;

pub trait ChangePasswordUseCase {
    fn change_password(&self, req: ChangePasswordRequest) -> ChangePasswordResult;
}

/// Changes the current password after verifying the existing credential.
pub struct ChangePasswordService<R: AuthAccountRepository> {
    auth_accounts: R,
    observability: Arc<dyn Observability + Send + Sync>,
}

impl<R: AuthAccountRepository> ChangePasswordService<R> {
    pub fn new(auth_accounts: R, observability: Arc<dyn Observability + Send + Sync>) -> Self {
        Self {
            auth_accounts,
            observability,
        }
    }
}

impl<R: AuthAccountRepository> ChangePasswordUseCase for ChangePasswordService<R> {
    fn change_password(&self, req: ChangePasswordRequest) -> ChangePasswordResult {
        let Some(mut account) = self.auth_accounts.get_by_username(&req.username) else {
            self.observability.emit(AuthBusinessEvent::PasswordChangeFailed {
                username: req.username,
                reason: PasswordChangeFailureReason::UnknownUsername,
            });
            return Err(ChangePasswordError::InvalidCredentials);
        };

        if account.is_locked {
            self.observability.emit(AuthBusinessEvent::PasswordChangeFailed {
                username: account.username.clone(),
                reason: PasswordChangeFailureReason::AccountLocked,
            });
            return Err(ChangePasswordError::AccountLocked);
        }

        if account.password_hash != req.current_password {
            self.observability.emit(AuthBusinessEvent::PasswordChangeFailed {
                username: account.username.clone(),
                reason: PasswordChangeFailureReason::InvalidCurrentPassword,
            });
            return Err(ChangePasswordError::InvalidCredentials);
        }

        if account.password_hash == req.new_password {
            self.observability.emit(AuthBusinessEvent::PasswordChangeFailed {
                username: account.username.clone(),
                reason: PasswordChangeFailureReason::PasswordReuseNotAllowed,
            });
            return Err(ChangePasswordError::PasswordReuseNotAllowed);
        }

        let account_id = account.id;
        let username = account.username.clone();
        account.set_password(req.new_password);
        self.auth_accounts.save(account);
        self.observability.emit(AuthBusinessEvent::PasswordChanged {
            account_id,
            username,
        });

        Ok(ChangePasswordResponse {
            message: "Password changed".to_string(),
        })
    }
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

        fn get_by_reset_token(&self, _token: &str) -> Option<AuthAccount> {
            None
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

    fn make_request(current_password: &str, new_password: &str) -> ChangePasswordRequest {
        ChangePasswordRequest {
            username: "demo".to_string(),
            current_password: current_password.to_string(),
            new_password: new_password.to_string(),
        }
    }

    #[test]
    fn change_password_succeeds_for_valid_current_password() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", false))),
        };
        let service = ChangePasswordService::new(repo, observability.clone());

        let result = service.change_password(make_request("password", "new-password"));

        assert_eq!(
            result,
            Ok(ChangePasswordResponse {
                message: "Password changed".to_string(),
            })
        );
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordChanged {
                account_id: 1,
                username: "demo".to_string(),
            }]
        );
    }

    #[test]
    fn change_password_fails_when_current_password_is_wrong() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", false))),
        };
        let service = ChangePasswordService::new(repo, observability.clone());

        let result = service.change_password(make_request("wrong-password", "new-password"));

        assert_eq!(result, Err(ChangePasswordError::InvalidCredentials));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordChangeFailed {
                username: "demo".to_string(),
                reason: PasswordChangeFailureReason::InvalidCurrentPassword,
            }]
        );
    }

    #[test]
    fn change_password_fails_for_locked_account() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", true))),
        };
        let service = ChangePasswordService::new(repo, observability.clone());

        let result = service.change_password(make_request("password", "new-password"));

        assert_eq!(result, Err(ChangePasswordError::AccountLocked));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordChangeFailed {
                username: "demo".to_string(),
                reason: PasswordChangeFailureReason::AccountLocked,
            }]
        );
    }

    #[test]
    fn change_password_rejects_reusing_the_existing_password() {
        let observability = test_observability();
        let repo = TestAuthAccountRepository {
            account: RefCell::new(Some(AuthAccount::new(1, "demo", "password", false))),
        };
        let service = ChangePasswordService::new(repo, observability.clone());

        let result = service.change_password(make_request("password", "password"));

        assert_eq!(result, Err(ChangePasswordError::PasswordReuseNotAllowed));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::PasswordChangeFailed {
                username: "demo".to_string(),
                reason: PasswordChangeFailureReason::PasswordReuseNotAllowed,
            }]
        );
    }
}
