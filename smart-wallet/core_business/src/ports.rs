#![forbid(unsafe_code)]

use async_trait::async_trait;
use crate::domain::user::{UserId, User, CreateUserDto};
use crate::domain::wallet::Wallet;
use crate::domain::asset::Asset;
use crate::domain::order::Order;
use crate::errors::BusinessError;

/// Persistence contract for users.
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait UserRepository: Send + Sync {
    async fn create(&self, dto: CreateUserDto) -> Result<User, BusinessError>;
    async fn find_by_email(&self, email: &str) -> Result<User, BusinessError>;
    async fn find_by_id(&self, id: &UserId) -> Result<User, BusinessError>;
}

/// Persistence contract for wallets.
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait WalletRepository: Send + Sync {
    async fn find(&self, user_id: &UserId) -> Result<Wallet, BusinessError>;
    async fn save(&self, wallet: &Wallet) -> Result<(), BusinessError>;
    async fn order_history(&self, user_id: &UserId) -> Result<Vec<Order>, BusinessError>;
    async fn record_order(&self, order: &Order) -> Result<(), BusinessError>;
}

/// Contract for asset price queries.
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait AssetRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<Asset>, BusinessError>;
    async fn find_price(&self, symbol: &str) -> Result<Asset, BusinessError>;
}
