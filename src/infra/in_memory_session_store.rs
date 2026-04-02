use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::auth_account::AuthAccount;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Revoked,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub session_id: String,
    pub account_id: u64,
    pub username: String,
    pub status: SessionStatus,
    pub created_at: u64,
    pub expires_at: u64,
    pub revoked_at: Option<u64>,
}

#[derive(Debug, Default)]
struct SessionState {
    next_session_id: AtomicU64,
    sessions: Mutex<HashMap<String, SessionRecord>>,
}

/// In-memory session storage used by the current auth boundary.
#[derive(Debug, Clone, Default)]
pub struct InMemorySessionStore {
    state: Arc<SessionState>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&self, account: &AuthAccount, ttl_seconds: u64) -> SessionRecord {
        let now = unix_timestamp();
        let session_id = format!(
            "session-{}",
            self.state.next_session_id.fetch_add(1, Ordering::Relaxed) + 1
        );
        let session = SessionRecord {
            session_id: session_id.clone(),
            account_id: account.id,
            username: account.username.clone(),
            status: SessionStatus::Active,
            created_at: now,
            expires_at: now + ttl_seconds,
            revoked_at: None,
        };

        self.state
            .sessions
            .lock()
            .expect("session store lock should not be poisoned")
            .insert(session_id, session.clone());

        session
    }

    pub fn get(&self, session_id: &str) -> Option<SessionRecord> {
        self.state
            .sessions
            .lock()
            .expect("session store lock should not be poisoned")
            .get(session_id)
            .cloned()
    }

    pub fn revoke(&self, session_id: &str) -> bool {
        let mut sessions = self
            .state
            .sessions
            .lock()
            .expect("session store lock should not be poisoned");

        let Some(session) = sessions.get_mut(session_id) else {
            return false;
        };

        session.status = SessionStatus::Revoked;
        session.revoked_at = Some(unix_timestamp());
        true
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}
