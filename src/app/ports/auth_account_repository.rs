use crate::domain::auth_account::AuthAccount;

/// Reads auth accounts needed by the login use case.
pub trait AuthAccountRepository {
    /// Returns the auth account for the supplied username, if one exists.
    fn get_by_username(&self, username: &str) -> Option<AuthAccount>;
}
