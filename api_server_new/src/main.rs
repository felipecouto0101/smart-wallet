#![forbid(unsafe_code)]

use color_eyre::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing_subscriber::{fmt, EnvFilter};

use api_server::routes::create_router;
use api_server::state::AppState;
use infra_data::{
    connect,
    repositories::{
        AssetRepositoryPg, FinancialProfileRepositoryPg, UserRepositoryPg, WalletRepositoryPg,
    },
    update_prices, LivePriceProvider,
};

const PRICE_REFRESH_SECS: u64 = 2;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    color_eyre::install()?;

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let pool = connect(&database_url).await?;

    let asset_repo = AssetRepositoryPg::new();
    let price_cache = asset_repo.cache_handle();

    // Fetch live prices once before starting the server
    let provider = LivePriceProvider::new();
    update_prices(&price_cache, &provider).await;

    // Background task: refresh prices every PRICE_REFRESH_SECS seconds
    let bg_cache = Arc::clone(&price_cache);
    tokio::spawn(async move {
        let provider = LivePriceProvider::new();
        let mut ticker = interval(Duration::from_secs(PRICE_REFRESH_SECS));
        ticker.tick().await; // skip the immediate first tick (already fetched above)
        loop {
            ticker.tick().await;
            update_prices(&bg_cache, &provider).await;
        }
    });

    let state = Arc::new(AppState {
        user_repo: Arc::new(UserRepositoryPg::new(pool.clone())),
        wallet_repo: Arc::new(WalletRepositoryPg::new(pool.clone())),
        asset_repo: Arc::new(asset_repo),
        profile_repo: Arc::new(FinancialProfileRepositoryPg::new(pool.clone())),
        jwt_secret,
    });

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8090").await?;
    tracing::info!("🚀 smart-wallet running at http://localhost:8090");
    axum::serve(listener, app).await?;

    Ok(())
}
