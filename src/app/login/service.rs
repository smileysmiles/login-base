use crate::app::ports::{AuthAccountRepository, ComplianceService};

use super::commands::LoginRequest;
use super::errors::LoginError;
use super::results::{LoginResponse, LoginResult};

/// Application-facing interface for executing the login flow.
pub trait LoginUseCase {
    /// Attempts to authenticate an auth account using the supplied credentials.
    fn login(&self, req: LoginRequest) -> LoginResult;
}

/// Default login use case implementation.
pub struct LoginService<R: AuthAccountRepository, C: ComplianceService> {
    auth_accounts: R,
    compliance: C,
}

impl<R: AuthAccountRepository, C: ComplianceService> LoginService<R, C> {
    /// Creates a login service from its required ports.
    pub fn new(auth_accounts: R, compliance: C) -> Self {
        Self {
            auth_accounts,
            compliance,
        }
    }
}

impl<R: AuthAccountRepository, C: ComplianceService> LoginUseCase for LoginService<R, C> {
    fn login(&self, req: LoginRequest) -> LoginResult {
        let Some(account) = self.auth_accounts.get_by_username(&req.username) else {
            return Err(LoginError::InvalidCredentials);
        };

        if account.is_locked {
            return Err(LoginError::AccountLocked);
        }

        // Temporary plain-text check until hashing is introduced.
        if req.password != account.password_hash {
            return Err(LoginError::InvalidCredentials);
        }

        if self.compliance.is_excluded(account.id) {
            return Err(LoginError::SelfExcluded);
        }

        Ok(LoginResponse {
            message: "OK".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::auth_account::AuthAccount;

    use super::*;

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
    }

    struct TestComplianceService {
        excluded: bool,
    }

    impl ComplianceService for TestComplianceService {
        fn is_excluded(&self, _player_id: u64) -> bool {
            self.excluded
        }
    }

    fn make_request(password: &str) -> LoginRequest {
        LoginRequest {
            username: "demo".to_string(),
            password: password.to_string(),
        }
    }

    #[test]
    fn login_succeeds_for_valid_credentials() {
        let service = LoginService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.login(make_request("password"));

        assert!(matches!(result, Ok(LoginResponse { message }) if message == "OK"));
    }

    #[test]
    fn login_fails_when_username_is_unknown() {
        let service = LoginService::new(
            TestAuthAccountRepository { account: None },
            TestComplianceService { excluded: false },
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::InvalidCredentials));
    }

    #[test]
    fn login_fails_when_password_is_wrong() {
        let service = LoginService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.login(make_request("wrong-password"));

        assert_eq!(result, Err(LoginError::InvalidCredentials));
    }

    #[test]
    fn login_fails_when_account_is_locked() {
        let service = LoginService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", true)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::AccountLocked));
    }

    #[test]
    fn login_fails_when_auth_account_is_self_excluded() {
        let service = LoginService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: true },
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::SelfExcluded));
    }
}
