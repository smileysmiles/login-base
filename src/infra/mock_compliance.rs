use crate::app::login::ComplianceService;

pub struct MockComplianceService;

impl ComplianceService for MockComplianceService {
    fn is_excluded(&self, _player_id: u64) -> bool {
        false
    }
}
