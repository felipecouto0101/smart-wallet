#![forbid(unsafe_code)]

pub mod middleware;

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use core_business::BusinessError;

/// JWT claims payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User UUID
    pub exp: usize,
}

pub const JWT_COOKIE: &str = "smart_wallet_token";

/// Generates a JWT valid for 24 hours.
pub fn generate_token(user_id: &Uuid, secret: &str) -> Result<String, BusinessError> {
    let expiry = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims { sub: user_id.to_string(), exp: expiry };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| BusinessError::InvalidToken)
}

/// Validates and decodes a JWT.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims, BusinessError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| BusinessError::InvalidToken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim::assert_ok;
    use pretty_assertions::assert_eq;

    #[test]
    fn token_generated_and_validated_successfully() {
        let id = Uuid::new_v4();
        let secret = "test_secret_long_enough_32bytes!";
        let token = assert_ok!(generate_token(&id, secret));
        let claims = assert_ok!(validate_token(&token, secret));
        assert_eq!(claims.sub, id.to_string());
    }

    #[test]
    fn invalid_token_returns_error() {
        let result = validate_token("invalid.token.here", "secret");
        assert!(result.is_err());
    }
}
