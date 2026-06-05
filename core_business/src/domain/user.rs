#![forbid(unsafe_code)]

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Newtype for user ID — prevents parameter confusion attacks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

/// Input payload for user creation with strict validation.
#[derive(Debug, Validate, Deserialize)]
pub struct CreateUserDto {
    #[validate(length(min = 2, max = 100))]
    pub name: String,

    #[validate(email)]
    pub email: String,

    /// Password is never stored as a plain String — uses secrecy.
    #[serde(skip)]
    pub password: SecretString,
}

/// Persisted user entity (without plain password).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: String,
    pub email: String,
    /// Bcrypt hash — never exposed in logs.
    #[serde(skip)]
    pub password_hash: SecretString,
}

impl User {
    pub fn new(id: UserId, name: String, email: String, password_hash: SecretString) -> Self {
        Self {
            id,
            name,
            email,
            password_hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use secrecy::ExposeSecret;

    #[test]
    fn user_id_generates_unique_uuids() {
        let id1 = UserId::new();
        let id2 = UserId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn user_preserves_fields() {
        let id = UserId::new();
        let hash = SecretString::from("bcrypt_hash");
        let user = User::new(
            id.clone(),
            "Alice".to_string(),
            "alice@email.com".to_string(),
            hash,
        );
        assert_eq!(user.name, "Alice");
        assert_eq!(user.email, "alice@email.com");
        assert_eq!(user.id, id);
    }

    #[test]
    fn password_hash_does_not_leak_in_debug() {
        let hash = SecretString::from("secret_password");
        let debug_str = format!("{:?}", hash);
        assert!(!debug_str.contains("secret_password"));
    }

    #[test]
    fn password_exposed_only_explicitly() {
        let hash = SecretString::from("my_password");
        assert_eq!(hash.expose_secret(), "my_password");
    }
}
