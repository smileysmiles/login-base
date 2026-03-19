/// An auth account used by the login flow.
#[derive(Debug, Clone)]
pub struct AuthAccount {
    /// Stable identifier used by downstream checks.
    pub id: u64,
    /// Login name submitted by the client.
    pub username: String,
    /// Stored password value used by the current plain-text comparison.
    pub password_hash: String,
    /// Indicates whether the account is locked internally.
    pub is_locked: bool,
}

impl AuthAccount {
    /// Creates an auth account record for the current in-memory store.
    pub fn new(
        id: u64,
        username: impl Into<String>,
        password_hash: impl Into<String>,
        is_locked: bool,
    ) -> Self {
        Self {
            id,
            username: username.into(),
            password_hash: password_hash.into(),
            is_locked,
        }
    }
}
