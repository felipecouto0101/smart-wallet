#![forbid(unsafe_code)]

use sqlx::PgPool;

/// Establishes the PostgreSQL connection pool and runs migrations.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(database_url).await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
    Ok(pool)
}
