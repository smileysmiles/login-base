use crate::app::ports::{AuthAccountRepository, ComplianceService};
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
}

impl<R: AuthAccountRepository, C: ComplianceService> AuthenticationService<R, C> {
    /// Creates an authentication service from its required ports.
    pub fn new(auth_accounts: R, compliance: C) -> Self {
        Self {
            auth_accounts,
            compliance,
        }
    }
}

impl<R: AuthAccountRepository, C: ComplianceService> AuthenticateUseCase
    for AuthenticationService<R, C>
{
    fn authenticate(&self, req: LoginRequest) -> Result<AuthenticationSuccess, LoginError> {
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

        Ok(AuthenticationSuccess { account })
    }
}

#[cfg(test)]
mod tests {
    use crate::app::ports::AuthAccountRepository;
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
        fn is_excluded(&self, _account_id: u64) -> bool {
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
    fn authenticate_succeeds_for_valid_credentials() {
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Ok(AuthenticationSuccess { account }) if account.id == 1));
    }

    #[test]
    fn authenticate_fails_when_username_is_unknown() {
        let service = AuthenticationService::new(
            TestAuthAccountRepository { account: None },
            TestComplianceService { excluded: false },
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
    }

    #[test]
    fn authenticate_fails_when_password_is_wrong() {
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.authenticate(make_request("wrong-password"));

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
    }

    #[test]
    fn authenticate_fails_when_account_is_locked() {
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", true)),
            },
            TestComplianceService { excluded: false },
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::AccountLocked)));
    }

    #[test]
    fn authenticate_fails_when_auth_account_is_self_excluded() {
        let service = AuthenticationService::new(
            TestAuthAccountRepository {
                account: Some(AuthAccount::new(1, "demo", "password", false)),
            },
            TestComplianceService { excluded: true },
        );

        let result = service.authenticate(make_request("password"));

        assert!(matches!(result, Err(LoginError::SelfExcluded)));
    }
}
