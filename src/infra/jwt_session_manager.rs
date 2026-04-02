use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::Deserialize;

use crate::app::ports::{SessionUser, TokenSessionManager};

use super::in_memory_session_store::{InMemorySessionStore, SessionStatus};

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct Claims {
    sid: String,
    sub: String,
    account_id: u64,
    exp: u64,
}

/// In-memory JWT validator and revocation list for local session management.
pub struct JwtSessionManager {
    decoding_key: DecodingKey,
    validation: Validation,
    session_store: InMemorySessionStore,
}

impl JwtSessionManager {
    /// Creates a JWT session manager using the supplied shared secret.
    pub fn new(secret: impl AsRef<[u8]>, session_store: InMemorySessionStore) -> Self {
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            validation: Validation::new(Algorithm::HS256),
            session_store,
        }
    }

    fn decode_active_claims(&self, token: &str) -> Option<Claims> {
        let claims = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .ok()?
            .claims;
        let session = self.session_store.get(&claims.sid)?;

        if session.status != SessionStatus::Active {
            return None;
        }

        if session.account_id != claims.account_id || session.username != claims.sub {
            return None;
        }

        if session.expires_at < claims.exp {
            return None;
        }

        Some(claims)
    }
}

impl TokenSessionManager for JwtSessionManager {
    fn is_active(&self, token: &str) -> bool {
        self.decode_active_claims(token).is_some()
    }

    fn current_user(&self, token: &str) -> Option<SessionUser> {
        let claims = self.decode_active_claims(token)?;
        let session = self.session_store.get(&claims.sid)?;

        Some(SessionUser {
            account_id: session.account_id,
            username: session.username,
        })
    }

    fn revoke(&self, token: &str) {
        let Some(claims) = decode::<Claims>(token, &self.decoding_key, &self.validation)
            .ok()
            .map(|decoded| decoded.claims)
        else {
            return;
        };

        self.session_store.revoke(&claims.sid);
    }
}

#[cfg(test)]
mod tests {
    use crate::app::ports::TokenIssuer;
    use crate::domain::auth_account::AuthAccount;
    use crate::infra::jwt_token_issuer::JwtTokenIssuer;

    use super::*;

    // These tests cover the session boundary itself: accepted tokens, revoked
    // tokens, malformed tokens, and identity resolution from an active session.
    #[test]
    fn accepts_valid_active_token() {
        let secret = "test-secret";
        let session_store = InMemorySessionStore::new();
        let issuer = JwtTokenIssuer::new(secret, 3600, session_store.clone());
        let manager = JwtSessionManager::new(secret, session_store);
        let token = issuer.issue_for(&AuthAccount::new(1, "demo", "password", false));

        assert!(manager.is_active(&token));
    }

    #[test]
    fn rejects_revoked_token() {
        let secret = "test-secret";
        let session_store = InMemorySessionStore::new();
        let issuer = JwtTokenIssuer::new(secret, 3600, session_store.clone());
        let manager = JwtSessionManager::new(secret, session_store);
        let token = issuer.issue_for(&AuthAccount::new(1, "demo", "password", false));

        manager.revoke(&token);

        assert!(!manager.is_active(&token));
    }

    #[test]
    fn rejects_invalid_token() {
        let manager = JwtSessionManager::new("test-secret", InMemorySessionStore::new());

        assert!(!manager.is_active("not-a-jwt"));
    }

    #[test]
    fn returns_current_user_for_active_token() {
        let secret = "test-secret";
        let session_store = InMemorySessionStore::new();
        let issuer = JwtTokenIssuer::new(secret, 3600, session_store.clone());
        let manager = JwtSessionManager::new(secret, session_store);
        let token = issuer.issue_for(&AuthAccount::new(42, "demo", "password", false));

        let user = manager.current_user(&token).unwrap();

        assert_eq!(user.account_id, 42);
        assert_eq!(user.username, "demo");
    }
}
