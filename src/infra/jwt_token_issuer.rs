use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{EncodingKey, Header, encode};
use serde::Serialize;

use crate::app::ports::TokenIssuer;
use crate::domain::auth_account::AuthAccount;

#[derive(Debug, Serialize)]
struct Claims {
    sub: String,
    account_id: u64,
    exp: u64,
}

/// JWT token issuer for the current auth boundary.
pub struct JwtTokenIssuer {
    encoding_key: EncodingKey,
    ttl_seconds: u64,
}

impl JwtTokenIssuer {
    /// Creates a JWT issuer using the supplied shared secret and TTL.
    pub fn new(secret: impl AsRef<[u8]>, ttl_seconds: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_ref()),
            ttl_seconds,
        }
    }
}

impl TokenIssuer for JwtTokenIssuer {
    fn issue_for(&self, account: &AuthAccount) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_secs();

        let claims = Claims {
            sub: account.username.clone(),
            account_id: account.id,
            exp: now + self.ttl_seconds,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .expect("jwt encoding should succeed for a valid shared secret")
    }
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, Clone)]
    struct DecodedClaims {
        sub: String,
        account_id: u64,
        exp: u64,
    }

    #[test]
    fn issues_jwt_for_authenticated_account() {
        let issuer = JwtTokenIssuer::new("test-secret", 3600);
        let token = issuer.issue_for(&AuthAccount::new(42, "demo", "password", false));

        let decoded = decode::<DecodedClaims>(
            &token,
            &DecodingKey::from_secret("test-secret".as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .unwrap();

        assert_eq!(decoded.claims.sub, "demo");
        assert_eq!(decoded.claims.account_id, 42);
        assert!(decoded.claims.exp > 0);
    }
}
