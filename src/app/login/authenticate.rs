use std::sync::Arc;

use crate::app::ports::{
    AuthAccountRepository, AuthBusinessEvent, ComplianceService, LoginFailureReason,
    Observability,
};
use crate::domain::auth_account::AuthAccount;

use super::commands::LoginRequest;
use super::errors::LoginError;

/// Successful internal authentication outcome.
#[derive(Debug, Clone)]
pub struct AuthenticationSuccess {
    pub account: AuthAccount,
}

/// Application-facing interface for the core authentication decision.
pub trait AuthenticateUseCase {
    fn authenticate(&self, req: LoginRequest) -> Result<AuthenticationSuccess, LoginError>;
}

/// Core authentication decision service for username and password login.
pub struct AuthenticationService<R: AuthAccountRepository, C: ComplianceService> {
    auth_accounts: R,
    compliance: C,
    observability: Arc<dyn Observability + Send + Sync>,
}

const MAX_FAILED_LOGIN_ATTEMPTS: u32 = 3;

impl<R: AuthAccountRepository, C: ComplianceService> AuthenticationService<R, C> {
    /// Creates an authentication service from its required ports.
    pub fn new(
        auth_accounts: R,
        compliance: C,
        observability: Arc<dyn Observability + Send + Sync>,
    ) -> Self {
        Self {
            auth_accounts,
            compliance,
            observability,
        }
    }
}

impl<R: AuthAccountRepository, C: ComplianceService> AuthenticateUseCase
    for AuthenticationService<R, C>
{
    fn authenticate(&self, req: LoginRequest) -> Result<AuthenticationSuccess, LoginError> {
        let username = req.username.clone();
        let Some(mut account) = self.auth_accounts.get_by_username(&req.username) else {
            self.observability.emit(AuthBusinessEvent::LoginFailed {
                username,
                reason: LoginFailureReason::UnknownUsername,
            });
            return Err(LoginError::InvalidCredentials);
        };

        if account.is_locked {
            self.observability.emit(AuthBusinessEvent::LoginFailed {
                username: account.username.clone(),
                reason: LoginFailureReason::AccountLocked,
            });
            return Err(LoginError::AccountLocked);
        }

        // Temporary plain-text check until hashing is introduced.
        if req.password != account.password_hash {
            account.record_failed_login_attempt(MAX_FAILED_LOGIN_ATTEMPTS);
            let is_now_locked = account.is_locked;
            let account_id = account.id;
            let failed_attempts = account.failed_login_attempts;
            self.auth_accounts.save(account);

            self.observability.emit(AuthBusinessEvent::LoginFailed {
                username: username.clone(),
                reason: LoginFailureReason::WrongPassword,
            });

            if is_now_locked {
                self.observability.emit(AuthBusinessEvent::AccountLocked {
                    account_id,
                    username,
                    failed_attempts,
                });
            }

            return Err(if is_now_locked {
                LoginError::AccountLocked
            } else {
                LoginError::InvalidCredentials
            });
        }

        account.clear_failed_login_attempts();
        self.auth_accounts.save(account.clone());

        if self.compliance.is_excluded(account.id) {
            self.observability.emit(AuthBusinessEvent::LoginFailed {
                username: account.username.clone(),
                reason: LoginFailureReason::SelfExcluded,
            });
            return Err(LoginError::SelfExcluded);
        }

        self.observability.emit(AuthBusinessEvent::LoginSucceeded {
            account_id: account.id,
            username: account.username.clone(),
        });

        Ok(AuthenticationSuccess { account })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::app::ports::AuthAccountRepository;
    use crate::domain::auth_account::AuthAccount;

    use super::*;

    // Minimal repository double used to drive the authentication branches directly.
    struct TestAuthAccountRepository {
        account: Option<AuthAccount>,
    }

    impl AuthAccountRepository for TestAuthAccountRepository {
        fn get_by_username(&self, username: &str) -> Option<AuthAccount> {
            self.account
                .as_ref()
                .filter(|account| account.username == username)
                .cloned()
        }

        fn get_by_reset_token(&self, _token: &str) -> Option<AuthAccount> {
            None
        }

        fn save(&self, _account: AuthAccount) {}
    }

    // Compliance is reduced to a single exclusion flag so the tests can focus on
    // the application decision tree rather than external behavior.
    struct TestComplianceService {
        excluded: bool,
    }

    impl ComplianceService for TestComplianceService {
        fn is_excluded(&self, _account_id: u64) -> bool {
            self.excluded
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

    // All authentication tests use the same demo username and vary only the
    // password or backing doubles for the branch under test.
    fn make_request(password: &str) -> LoginRequest {
        LoginRequest {
            username: "demo".to_string(),
            password: password.to_string(),
        }
    }

    #[test]
    fn authenticate_succeeds_for_valid_credentials() {
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
            observability.clone(),
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Ok(AuthenticationSuccess { account }) if account.id == 1));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::LoginSucceeded {
                account_id: 1,
                username: "demo".to_string(),
            }]
        );
    }

    #[test]
    fn authenticate_fails_when_username_is_unknown() {
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository { account: None },
            TestComplianceService { excluded: false },
            observability.clone(),
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::LoginFailed {
                username: "demo".to_string(),
                reason: LoginFailureReason::UnknownUsername,
            }]
        );
    }

    #[test]
    fn authenticate_fails_when_password_is_wrong() {
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
            observability.clone(),
        );

        let result = service.authenticate(make_request("wrong-password"));

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::LoginFailed {
                username: "demo".to_string(),
                reason: LoginFailureReason::WrongPassword,
            }]
        );
    }

    #[test]
    fn authenticate_locks_account_after_three_wrong_password_attempts() {
        let mut account = AuthAccount::new(1, "demo", "password", false);
        account.failed_login_attempts = 2;
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(account),
            },
            TestComplianceService { excluded: false },
            observability.clone(),
        );

        let result = service.authenticate(make_request("wrong-password"));

        assert!(matches!(result, Err(LoginError::AccountLocked)));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [
                AuthBusinessEvent::LoginFailed {
                    username: "demo".to_string(),
                    reason: LoginFailureReason::WrongPassword,
                },
                AuthBusinessEvent::AccountLocked {
                    account_id: 1,
                    username: "demo".to_string(),
                    failed_attempts: 3,
                }
            ]
        );
    }

    #[test]
    fn authenticate_fails_when_account_is_locked() {
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", true)),
            },
            TestComplianceService { excluded: false },
            observability.clone(),
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::AccountLocked)));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::LoginFailed {
                username: "demo".to_string(),
                reason: LoginFailureReason::AccountLocked,
            }]
        );
    }

    #[test]
    fn authenticate_fails_when_auth_account_is_self_excluded() {
        let observability = test_observability();
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: true },
            observability.clone(),
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::SelfExcluded)));
        assert_eq!(
            observability
                .events
                .lock()
                .expect("observability mutex should be available")
                .as_slice(),
            [AuthBusinessEvent::LoginFailed {
                username: "demo".to_string(),
                reason: LoginFailureReason::SelfExcluded,
            }]
        );
    }
}
