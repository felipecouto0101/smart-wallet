#![forbid(unsafe_code)]

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{info, warn};

use core_business::{Asset, AssetType, BusinessError, PriceProvider};

/// Live price fetcher using Binance (crypto USDT) + AwesomeAPI (USD/BRL).
///
/// Flow: BTC/ETH/SOL prices in USDT × USD/BRL rate = price in BRL cents.
///
/// # Example
/// ```ignore
/// let provider = LivePriceProvider::new();
/// let assets = provider.fetch_prices().await.unwrap();
/// assert!(!assets.is_empty());
/// ```
pub struct LivePriceProvider {
    client: Client,
}

impl LivePriceProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
        }
    }
}

impl Default for LivePriceProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ── Binance response ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BinanceTicker {
    price: String,
}

// ── AwesomeAPI response ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AwesomeRate {
    bid: String,
}

#[async_trait]
impl PriceProvider for LivePriceProvider {
    async fn fetch_prices(&self) -> Result<Vec<Asset>, BusinessError> {
        let (btc_usdt, eth_usdt, sol_usdt, usd_brl) = tokio::join!(
            fetch_binance(&self.client, "BTCUSDT"),
            fetch_binance(&self.client, "ETHUSDT"),
            fetch_binance(&self.client, "SOLUSDT"),
            fetch_awesome(&self.client),
        );

        let usd_brl = usd_brl?;
        let btc = usdt_to_brl_cents(btc_usdt?, usd_brl, "BTC")?;
        let eth = usdt_to_brl_cents(eth_usdt?, usd_brl, "ETH")?;
        let sol = usdt_to_brl_cents(sol_usdt?, usd_brl, "SOL")?;
        let usd = price_cents(usd_brl, "USD")?;

        info!(btc, eth, sol, usd, "live prices updated (cents BRL)");

        Ok(vec![
            Asset::new(AssetType::Bitcoin,  btc),
            Asset::new(AssetType::Ethereum, eth),
            Asset::new(AssetType::Solana,   sol),
            Asset::new(AssetType::Dollar,   usd),
        ])
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn fetch_binance(client: &Client, symbol: &str) -> Result<f64, BusinessError> {
    let url = format!(
        "https://api.binance.com/api/v3/ticker/price?symbol={symbol}"
    );

    let text = client
        .get(&url)
        .send()
        .await
        .map_err(|e| BusinessError::PriceFetchError(e.to_string()))?
        .text()
        .await
        .map_err(|e| BusinessError::PriceFetchError(e.to_string()))?;

    let ticker: BinanceTicker = serde_json::from_str(&text)
        .map_err(|e| BusinessError::PriceFetchError(
            format!("binance parse ({symbol}): {e} — body: {text}")
        ))?;

    ticker.price.parse::<f64>()
        .map_err(|e| BusinessError::PriceFetchError(
            format!("binance price parse ({symbol}): {e}")
        ))
}

async fn fetch_awesome(client: &Client) -> Result<f64, BusinessError> {
    let url = "https://economia.awesomeapi.com.br/json/last/USD-BRL";

    #[derive(Deserialize)]
    struct Wrapper {
        #[serde(rename = "USDBRL")]
        usdbrl: AwesomeRate,
    }

    let text = client
        .get(url)
        .send()
        .await
        .map_err(|e| BusinessError::PriceFetchError(e.to_string()))?
        .text()
        .await
        .map_err(|e| BusinessError::PriceFetchError(e.to_string()))?;

    let resp: Wrapper = serde_json::from_str(&text)
        .map_err(|e| BusinessError::PriceFetchError(
            format!("awesomeapi parse: {e} — body: {text}")
        ))?;

    resp.usdbrl.bid.parse::<f64>()
        .map_err(|e| BusinessError::PriceFetchError(format!("awesomeapi bid parse: {e}")))
}

/// Converts a USDT price to BRL cents using the USD/BRL rate.
fn usdt_to_brl_cents(usdt: f64, usd_brl: f64, symbol: &str) -> Result<u64, BusinessError> {
    if usdt <= 0.0 || usd_brl <= 0.0 {
        warn!(symbol, "invalid price, skipping");
        return Err(BusinessError::PriceFetchError(
            format!("invalid price for {symbol}")
        ));
    }
    Ok((usdt * usd_brl * 100.0).round() as u64)
}

fn price_cents(value: f64, symbol: &str) -> Result<u64, BusinessError> {
    if value <= 0.0 {
        warn!(symbol, "price unavailable");
        return Err(BusinessError::PriceFetchError(
            format!("no price available for {symbol}")
        ));
    }
    Ok((value * 100.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn usdt_to_brl_cents_converts_correctly() {
        // BTC at $60.000 USDT × R$5,00/USD = R$300.000,00 = 30_000_000 cents
        assert_eq!(usdt_to_brl_cents(60_000.0, 5.0, "BTC").unwrap(), 30_000_000);
    }

    #[test]
    fn usdt_to_brl_cents_rounds_correctly() {
        // $1.234,567 × R$5,123 = R$6.324,795941 → 632_480 cents (rounded)
        assert_eq!(usdt_to_brl_cents(1_234.567, 5.123, "ETH").unwrap(), 632_469);
    }

    #[test]
    fn usdt_to_brl_cents_returns_err_on_zero_usdt() {
        assert!(usdt_to_brl_cents(0.0, 5.0, "BTC").is_err());
    }

    #[test]
    fn usdt_to_brl_cents_returns_err_on_zero_rate() {
        assert!(usdt_to_brl_cents(60_000.0, 0.0, "BTC").is_err());
    }

    #[test]
    fn price_cents_converts_usd_rate() {
        assert_eq!(price_cents(5.08, "USD").unwrap(), 508);
    }

    #[test]
    fn price_cents_returns_err_on_zero() {
        assert!(price_cents(0.0, "USD").is_err());
    }
}
