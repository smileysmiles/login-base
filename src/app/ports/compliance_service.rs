/// Performs compliance checks required by the login use case.
pub trait ComplianceService {
    /// Returns whether the player is currently excluded.
    fn is_excluded(&self, player_id: u64) -> bool;
}
