#![forbid(unsafe_code)]

use core_business::ports::{AssetRepository, UserRepository, WalletRepository};
use std::sync::Arc;

pub use core_business::ports::FinancialProfileRepository;

#[cfg(any(test, feature = "test-helpers"))]
pub use core_business::ports::MockFinancialProfileRepository;

/// Shared application state injected into Axum routes.
pub struct AppState {
    pub user_repo: Arc<dyn UserRepository>,
    pub wallet_repo: Arc<dyn WalletRepository>,
    pub asset_repo: Arc<dyn AssetRepository>,
    pub profile_repo: Arc<dyn FinancialProfileRepository>,
    pub jwt_secret: String,
}
