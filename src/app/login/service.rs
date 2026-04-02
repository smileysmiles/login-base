use crate::app::ports::TokenIssuer;

use super::authenticate::AuthenticateUseCase;
use super::commands::LoginRequest;
use super::results::{LoginResponse, LoginResult};

/// Application-facing interface for executing the login flow.
pub trait LoginUseCase {
    /// Attempts to authenticate an auth account and issue a JWT on success.
    fn login(&self, req: LoginRequest) -> LoginResult;
}

/// Thin login wrapper that issues a token after successful authentication.
pub struct LoginService<A: AuthenticateUseCase, T: TokenIssuer> {
    authenticator: A,
    token_issuer: T,
}

impl<A: AuthenticateUseCase, T: TokenIssuer> LoginService<A, T> {
    /// Creates a login service from its required authentication and token ports.
    pub fn new(authenticator: A, token_issuer: T) -> Self {
        Self {
            authenticator,
            token_issuer,
        }
    }
}

impl<A: AuthenticateUseCase, T: TokenIssuer> LoginUseCase for LoginService<A, T> {
    fn login(&self, req: LoginRequest) -> LoginResult {
        let authenticated = self.authenticator.authenticate(req)?;
        let token = self.token_issuer.issue_for(&authenticated.account);

        Ok(LoginResponse {
            account_id: authenticated.account.id,
            username: authenticated.account.username,
            message: "OK".to_string(),
            token,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::app::ports::TokenIssuer;
    use crate::domain::auth_account::AuthAccount;

    use super::*;
    use super::super::authenticate::{AuthenticateUseCase, AuthenticationSuccess};
    use super::super::errors::LoginError;

    // Authentication is stubbed so these tests stay focused on login orchestration:
    // propagate auth outcomes and issue a token only on success.
    struct StubAuthenticateUseCase {
        result: Result<AuthenticationSuccess, LoginError>,
    }

    impl AuthenticateUseCase for StubAuthenticateUseCase {
        fn authenticate(&self, _req: LoginRequest) -> Result<AuthenticationSuccess, LoginError> {
            self.result.clone()
        }
    }

    // The issuer double makes token creation deterministic for assertions.
    struct StubTokenIssuer;

    impl TokenIssuer for StubTokenIssuer {
        fn issue_for(&self, account: &AuthAccount) -> String {
            format!("token-for-{}", account.id)
        }
    }

    // Service tests keep the request shape fixed and swap the auth result instead.
    fn make_request(password: &str) -> LoginRequest {
        LoginRequest {
            username: "demo".to_string(),
            password: password.to_string(),
        }
    }

    #[test]
    fn login_succeeds_for_valid_credentials() {
        let service = LoginService::new(
            StubAuthenticateUseCase {
                result: Ok(AuthenticationSuccess {
                    account: AuthAccount::new(1, "demo", "password", false),
                }),
            },
            StubTokenIssuer,
        );

        let result = service.login(make_request("password"));

        assert!(matches!(
            result,
            Ok(LoginResponse {
                account_id,
                username,
                message,
                token
            }) if account_id == 1
                && username == "demo"
                && message == "OK"
                && token == "token-for-1"
        ));
    }

    #[test]
    fn login_fails_when_username_is_unknown() {
        let service = LoginService::new(
            StubAuthenticateUseCase {
                result: Err(LoginError::InvalidCredentials),
            },
            StubTokenIssuer,
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::InvalidCredentials));
    }

    #[test]
    fn login_fails_when_password_is_wrong() {
        let service = LoginService::new(
            StubAuthenticateUseCase {
                result: Err(LoginError::InvalidCredentials),
            },
            StubTokenIssuer,
        );

        let result = service.login(make_request("wrong-password"));

        assert_eq!(result, Err(LoginError::InvalidCredentials));
    }

    #[test]
    fn login_fails_when_account_is_locked() {
        let service = LoginService::new(
            StubAuthenticateUseCase {
                result: Err(LoginError::AccountLocked),
            },
            StubTokenIssuer,
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::AccountLocked));
    }

    #[test]
    fn login_fails_when_auth_account_is_self_excluded() {
        let service = LoginService::new(
            StubAuthenticateUseCase {
                result: Err(LoginError::SelfExcluded),
            },
            StubTokenIssuer,
        );

        let result = service.login(make_request("password"));

        assert_eq!(result, Err(LoginError::SelfExcluded));
    }
}
