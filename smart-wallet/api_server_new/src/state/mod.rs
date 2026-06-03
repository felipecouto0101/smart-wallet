#![forbid(unsafe_code)]

use std::sync::Arc;
use core_business::ports::{UserRepository, WalletRepository, AssetRepository};

/// Shared application state injected into Axum routes.
pub struct AppState {
    pub user_repo:   Arc<dyn UserRepository>,
    pub wallet_repo: Arc<dyn WalletRepository>,
    pub asset_repo:  Arc<dyn AssetRepository>,
    pub jwt_secret:  String,
}
