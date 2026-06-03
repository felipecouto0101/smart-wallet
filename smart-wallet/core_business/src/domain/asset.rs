#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Supported asset types in the wallet.
///
/// # Example
/// ```
/// use core_business::AssetType;
/// let asset = AssetType::Bitcoin;
/// assert_eq!(asset.symbol(), "BTC");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetType {
    Bitcoin,
    Ethereum,
    Solana,
    Dollar,
    Real,
}

impl AssetType {
    /// Returns the market symbol for this asset.
    pub fn symbol(&self) -> &'static str {
        match self {
            AssetType::Bitcoin  => "BTC",
            AssetType::Ethereum => "ETH",
            AssetType::Solana   => "SOL",
            AssetType::Dollar   => "USD",
            AssetType::Real     => "BRL",
        }
    }

    /// Returns the full name of the asset.
    pub fn name(&self) -> &'static str {
        match self {
            AssetType::Bitcoin  => "Bitcoin",
            AssetType::Ethereum => "Ethereum",
            AssetType::Solana   => "Solana",
            AssetType::Dollar   => "US Dollar",
            AssetType::Real     => "Brazilian Real",
        }
    }
}

/// An asset with its current price in cents (BRL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub asset_type: AssetType,
    /// Price in cents (BRL) to avoid floating point arithmetic.
    pub price_cents: u64,
}

impl Asset {
    pub fn new(asset_type: AssetType, price_cents: u64) -> Self {
        Self { asset_type, price_cents }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn bitcoin_symbol_is_btc() {
        assert_eq!(AssetType::Bitcoin.symbol(), "BTC");
    }

    #[test]
    fn ethereum_symbol_is_eth() {
        assert_eq!(AssetType::Ethereum.symbol(), "ETH");
    }

    #[test]
    fn solana_symbol_is_sol() {
        assert_eq!(AssetType::Solana.symbol(), "SOL");
    }

    #[test]
    fn dollar_symbol_is_usd() {
        assert_eq!(AssetType::Dollar.symbol(), "USD");
    }

    #[test]
    fn real_symbol_is_brl() {
        assert_eq!(AssetType::Real.symbol(), "BRL");
    }

    #[test]
    fn asset_new_preserves_price() {
        let asset = Asset::new(AssetType::Bitcoin, 30_000_000);
        assert_eq!(asset.price_cents, 30_000_000);
    }
}
