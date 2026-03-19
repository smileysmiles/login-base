/// Explicit internal login outcomes.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    /// Username was not found or password did not match.
    InvalidCredentials,
    /// Account is locked internally.
    AccountLocked,
    /// Account is excluded by the compliance check.
    SelfExcluded,
}
