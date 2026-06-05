#![forbid(unsafe_code)]

use async_trait::async_trait;
use bcrypt::{hash, verify, DEFAULT_COST};
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use core_business::{
    BusinessError, UserId, User,
    domain::user::CreateUserDto,
    ports::UserRepository,
};

pub struct UserRepositoryPg {
    pool: PgPool,
}

impl UserRepositoryPg {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    name: String,
    email: String,
    password_hash: String,
}

#[async_trait]
impl UserRepository for UserRepositoryPg {
    #[instrument(skip(self, dto))]
    async fn create(&self, dto: CreateUserDto) -> Result<User, BusinessError> {
        let password_hash = hash(dto.password.expose_secret(), DEFAULT_COST)
            .map_err(|_| BusinessError::InvalidCredentials)?;

        let id = Uuid::new_v4();

        // Begin transaction to ensure user + wallet are created atomically
        let mut tx = self.pool.begin().await.map_err(|_| BusinessError::UserNotFound)?;

        sqlx::query(
            "INSERT INTO users (id, name, email, password_hash) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.email)
        .bind(&password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") {
                BusinessError::DuplicateUser
            } else {
                BusinessError::UserNotFound
            }
        })?;

        // Create initial wallet with 100,000.00 BRL (10,000,000 cents)
        sqlx::query(
            "INSERT INTO wallets (user_id, balance_cents) VALUES ($1, $2)",
        )
        .bind(id)
        .bind(10_000_000_i64)
        .execute(&mut *tx)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        tx.commit().await.map_err(|_| BusinessError::UserNotFound)?;

        Ok(User::new(
            UserId(id),
            dto.name,
            dto.email,
            SecretString::from(password_hash),
        ))
    }

    #[instrument(skip(self))]
    async fn find_by_email(&self, email: &str) -> Result<User, BusinessError> {
        let row: UserRow = sqlx::query_as(
            "SELECT id, name, email, password_hash FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        Ok(User::new(
            UserId(row.id),
            row.name,
            row.email,
            SecretString::from(row.password_hash),
        ))
    }

    #[instrument(skip(self))]
    async fn find_by_id(&self, id: &UserId) -> Result<User, BusinessError> {
        let row: UserRow = sqlx::query_as(
            "SELECT id, name, email, password_hash FROM users WHERE id = $1",
        )
        .bind(id.0)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        Ok(User::new(
            UserId(row.id),
            row.name,
            row.email,
            SecretString::from(row.password_hash),
        ))
    }
}

/// Verifies a plain password against its bcrypt hash.
pub fn verify_password(password: &SecretString, hash: &SecretString) -> bool {
    verify(password.expose_secret(), hash.expose_secret()).unwrap_or(false)
}
