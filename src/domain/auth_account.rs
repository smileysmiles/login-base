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
    /// Counts consecutive password failures used for lockout.
    pub failed_login_attempts: u32,
    /// Active password reset token when a reset has been requested.
    pub password_reset_token: Option<String>,
    /// Expiry timestamp for the current password reset token.
    pub password_reset_expires_at: Option<u64>,
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
            failed_login_attempts: 0,
            password_reset_token: None,
            password_reset_expires_at: None,
        }
    }

    /// Records a password failure and locks the account once the threshold is hit.
    pub fn record_failed_login_attempt(&mut self, max_attempts: u32) {
        self.failed_login_attempts += 1;

        if self.failed_login_attempts >= max_attempts {
            self.is_locked = true;
        }
    }

    /// Clears password-failure state after successful credential proof.
    pub fn clear_failed_login_attempts(&mut self) {
        self.failed_login_attempts = 0;
    }

    /// Updates the stored password and removes any reset token state.
    pub fn set_password(&mut self, password_hash: impl Into<String>) {
        self.password_hash = password_hash.into();
        self.is_locked = false;
        self.clear_failed_login_attempts();
        self.clear_password_reset();
    }

    /// Stores a password reset token and expiry.
    pub fn issue_password_reset(&mut self, token: impl Into<String>, expires_at: u64) {
        self.password_reset_token = Some(token.into());
        self.password_reset_expires_at = Some(expires_at);
    }

    /// Clears the current password reset token.
    pub fn clear_password_reset(&mut self) {
        self.password_reset_token = None;
        self.password_reset_expires_at = None;
    }

    /// Returns `true` when the supplied reset token matches the current active token.
    pub fn can_reset_with(&self, token: &str, now: u64) -> bool {
        self.password_reset_token.as_deref() == Some(token)
            && self.password_reset_expires_at.is_some_and(|expires_at| now <= expires_at)
    }
}
