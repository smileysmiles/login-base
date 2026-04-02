use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::app::ports::AuthAccountRepository;
use crate::domain::auth_account::AuthAccount;

/// In-memory auth account repository used by the current executable and tests.
#[derive(Clone)]
pub struct InMemoryAuthAccountRepository {
    auth_accounts: Arc<Mutex<HashMap<String, AuthAccount>>>,
}

impl InMemoryAuthAccountRepository {
    /// Creates a repository seeded with a single demo user.
    #[allow(dead_code)]
    pub fn new_with_demo_user() -> Self {
        Self::new_with_demo_users(1)
    }

    /// Creates a repository seeded with `count` demo users for local and perf flows.
    pub fn new_with_demo_users(count: u64) -> Self {
        let auth_accounts = (1..=count)
            .map(|id| {
                let username = if id == 1 {
                    "demo".to_string()
                }
                else {
                    format!("demo-{}", id)
                };
                let account = AuthAccount::new(id, username.clone(), "password", false);
                (username, account)
            })
            .collect();

        Self {
            auth_accounts: Arc::new(Mutex::new(auth_accounts)),
        }
    }
}

impl AuthAccountRepository for InMemoryAuthAccountRepository {
    fn get_by_username(&self, username: &str) -> Option<AuthAccount> {
        self.auth_accounts
            .lock()
            .expect("auth account store lock should not be poisoned")
            .get(username)
            .cloned()
    }

    fn get_by_reset_token(&self, token: &str) -> Option<AuthAccount> {
        self.auth_accounts
            .lock()
            .expect("auth account store lock should not be poisoned")
            .values()
            .find(|account| account.password_reset_token.as_deref() == Some(token))
            .cloned()
    }

    fn save(&self, account: AuthAccount) {
        self.auth_accounts
            .lock()
            .expect("auth account store lock should not be poisoned")
            .insert(account.username.clone(), account);
    }
}
