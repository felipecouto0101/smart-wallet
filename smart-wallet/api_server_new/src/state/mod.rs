#![forbid(unsafe_code)]

use std::sync::Arc;
use core_business::ports::{UserRepository, WalletRepository, AssetRepository};
use infra_data::FinancialProfileRepositoryPg;

/// Shared application state injected into Axum routes.
pub struct AppState {
    pub user_repo:    Arc<dyn UserRepository>,
    pub wallet_repo:  Arc<dyn WalletRepository>,
    pub asset_repo:   Arc<dyn AssetRepository>,
    pub profile_repo: Arc<FinancialProfileRepositoryPg>,
    pub jwt_secret:   String,
}
