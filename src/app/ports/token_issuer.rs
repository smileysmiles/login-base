use crate::domain::auth_account::AuthAccount;

/// Issues a login token for a successfully authenticated account.
pub trait TokenIssuer {
    fn issue_for(&self, account: &AuthAccount) -> String;
}
