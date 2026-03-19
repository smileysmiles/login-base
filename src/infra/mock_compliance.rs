use crate::app::ports::ComplianceService;

/// Compliance adapter that never excludes any auth account.
pub struct MockComplianceService;

impl ComplianceService for MockComplianceService {
    fn is_excluded(&self, _player_id: u64) -> bool {
        false
    }
}
