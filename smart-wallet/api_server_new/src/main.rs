#![forbid(unsafe_code)]

mod auth;
mod routes;
mod state;

use color_eyre::Result;
use tracing_subscriber::{EnvFilter, fmt};
use std::sync::Arc;

use infra_data::{connect, repositories::{UserRepositoryPg, WalletRepositoryPg, AssetRepositoryPg}};
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    color_eyre::install()?;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set");

    let pool = connect(&database_url).await?;

    let state = Arc::new(AppState {
        user_repo:   Arc::new(UserRepositoryPg::new(pool.clone())),
        wallet_repo: Arc::new(WalletRepositoryPg::new(pool.clone())),
        asset_repo:  Arc::new(AssetRepositoryPg::new()),
        jwt_secret,
    });

    let app = routes::create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8090").await?;
    tracing::info!("🚀 smart-wallet running at http://localhost:8090");
    axum::serve(listener, app).await?;

    Ok(())
}
