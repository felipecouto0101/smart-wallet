#![forbid(unsafe_code)]

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use core_business::{ports::AssetRepository, Asset, AssetType, BusinessError, PriceProvider};

/// Thread-safe in-memory cache of asset prices.
/// Updated by the background task via `update_prices`.
pub struct AssetRepositoryPg {
    cache: Arc<RwLock<Vec<Asset>>>,
}

impl AssetRepositoryPg {
    /// Creates the repository with hardcoded fallback prices.
    /// The background task should call `update_prices` shortly after startup.
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(Self::fallback())),
        }
    }

    /// Returns a clone of the inner cache handle for use by the background task.
    pub fn cache_handle(&self) -> Arc<RwLock<Vec<Asset>>> {
        Arc::clone(&self.cache)
    }

    fn fallback() -> Vec<Asset> {
        vec![
            Asset::new(AssetType::Bitcoin, 35_000_000),
            Asset::new(AssetType::Ethereum, 1_800_000),
            Asset::new(AssetType::Solana, 70_000),
            Asset::new(AssetType::Dollar, 5_100),
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
        Ok(self.cache.read().await.clone())
    }

    async fn find_price(&self, symbol: &str) -> Result<Asset, BusinessError> {
        self.cache
            .read()
            .await
            .iter()
            .find(|a| a.asset_type.symbol() == symbol)
            .cloned()
            .ok_or_else(|| BusinessError::AssetNotFound(symbol.to_string()))
    }
}

/// Updates the cache using the given provider.
/// Called by the background task in `main.rs`.
pub async fn update_prices(cache: &Arc<RwLock<Vec<Asset>>>, provider: &dyn PriceProvider) {
    match provider.fetch_prices().await {
        Ok(prices) => {
            *cache.write().await = prices;
            tracing::info!("asset prices refreshed from live feed");
        }
        Err(e) => {
            tracing::warn!(error = %e, "price fetch failed, keeping cached prices");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::{assert_err, assert_ok};
    use mockall::mock;
    use pretty_assertions::assert_eq;

    mock! {
        PriceProvider {}
        #[async_trait::async_trait]
        impl core_business::PriceProvider for PriceProvider {
            async fn fetch_prices(&self) -> Result<Vec<Asset>, BusinessError>;
        }
    }

    fn mock_assets() -> Vec<Asset> {
        vec![
            Asset::new(AssetType::Bitcoin, 40_000_000),
            Asset::new(AssetType::Ethereum, 2_000_000),
            Asset::new(AssetType::Solana, 80_000),
            Asset::new(AssetType::Dollar, 5_500),
        ]
    }

    #[tokio::test]
    async fn list_all_returns_fallback_on_init() {
        let repo = AssetRepositoryPg::new();
        let assets = assert_ok!(repo.list_all().await);
        assert_eq!(assets.len(), 4);
    }

    #[tokio::test]
    async fn find_price_returns_btc() {
        let repo = AssetRepositoryPg::new();
        let asset = assert_ok!(repo.find_price("BTC").await);
        assert_eq!(asset.asset_type, AssetType::Bitcoin);
    }

    #[tokio::test]
    async fn find_price_returns_err_for_unknown() {
        let repo = AssetRepositoryPg::new();
        assert_err!(repo.find_price("XYZ").await);
    }

    #[tokio::test]
    async fn update_prices_replaces_cache() {
        let cache = Arc::new(RwLock::new(AssetRepositoryPg::fallback()));
        let mut mock = MockPriceProvider::new();
        mock.expect_fetch_prices()
            .once()
            .returning(|| Ok(mock_assets()));

        update_prices(&cache, &mock).await;

        let prices = cache.read().await;
        let btc = prices
            .iter()
            .find(|a| a.asset_type == AssetType::Bitcoin)
            .unwrap();
        assert_eq!(btc.price_cents, 40_000_000);
    }

    #[tokio::test]
    async fn update_prices_keeps_cache_on_error() {
        let cache = Arc::new(RwLock::new(AssetRepositoryPg::fallback()));
        let original_btc = cache
            .read()
            .await
            .iter()
            .find(|a| a.asset_type == AssetType::Bitcoin)
            .unwrap()
            .price_cents;

        let mut mock = MockPriceProvider::new();
        mock.expect_fetch_prices()
            .once()
            .returning(|| Err(BusinessError::PriceFetchError("timeout".into())));

        update_prices(&cache, &mock).await;

        let btc = cache
            .read()
            .await
            .iter()
            .find(|a| a.asset_type == AssetType::Bitcoin)
            .unwrap()
            .price_cents;
        assert_eq!(btc, original_btc);
    }
}
