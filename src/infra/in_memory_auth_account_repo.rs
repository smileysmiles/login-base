use crate::app::ports::AuthAccountRepository;
use crate::domain::auth_account::AuthAccount;

/// In-memory auth account repository used by the current executable and tests.
pub struct InMemoryAuthAccountRepository {
    auth_accounts: Vec<AuthAccount>,
}

impl InMemoryAuthAccountRepository {
    /// Creates a repository seeded with a single demo user.
    pub fn new_with_demo_user() -> Self {
        let demo = AuthAccount::new(1, "demo", "password", false);

        Self {
            auth_accounts: vec![demo],
        }
    }
}

impl AuthAccountRepository for InMemoryAuthAccountRepository {
    fn get_by_username(&self, username: &str) -> Option<AuthAccount> {
        self.auth_accounts
            .iter()
            .find(|account| account.username == username)
            .cloned()
    }
}
