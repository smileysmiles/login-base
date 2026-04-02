/// Identity resolved from a validated bearer token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionUser {
    pub account_id: u64,
    pub username: String,
}

/// Validates and revokes bearer tokens for session-style controls.
pub trait TokenSessionManager {
    /// Returns `true` when the token is structurally valid and still active.
    fn is_active(&self, token: &str) -> bool;

    /// Returns the authenticated identity for an active token.
    fn current_user(&self, token: &str) -> Option<SessionUser>;

    /// Revokes a valid token so future checks can reject it.
    fn revoke(&self, token: &str);
}
