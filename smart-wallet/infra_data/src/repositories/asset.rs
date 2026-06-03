#![forbid(unsafe_code)]

use async_trait::async_trait;
use core_business::{Asset, AssetType, BusinessError, ports::AssetRepository};

/// In-memory asset repository with simulated prices.
/// Replace with a live pricing API integration in production.
pub struct AssetRepositoryPg;

impl AssetRepositoryPg {
    pub fn new() -> Self {
        Self
    }

    fn available_assets() -> Vec<Asset> {
        vec![
            Asset::new(AssetType::Bitcoin,  35_000_000), // R$ 350.000,00
            Asset::new(AssetType::Ethereum,   1_800_000), // R$  18.000,00
            Asset::new(AssetType::Solana,        70_000), // R$     700,00
            Asset::new(AssetType::Dollar,         5_100), // R$      51,00
            Asset::new(AssetType::Real,            1_000), // R$      10,00
        ]
    }
}

impl Default for AssetRepositoryPg {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetRepository for AssetRepositoryPg {
    async fn list_all(&self) -> Result<Vec<Asset>, BusinessError> {
        Ok(Self::available_assets())
    }

    async fn find_price(&self, symbol: &str) -> Result<Asset, BusinessError> {
        Self::available_assets()
            .into_iter()
            .find(|a| a.asset_type.symbol() == symbol)
            .ok_or_else(|| BusinessError::AssetNotFound(symbol.to_string()))
    }
}
