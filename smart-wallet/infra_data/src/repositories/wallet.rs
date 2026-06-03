#![forbid(unsafe_code)]

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::instrument;
use std::collections::HashMap;

use core_business::{
    BusinessError, UserId,
    domain::wallet::{Wallet, Position},
    domain::asset::AssetType,
    domain::order::Order,
    ports::WalletRepository,
};

pub struct WalletRepositoryPg {
    pool: PgPool,
}

impl WalletRepositoryPg {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct WalletRow {
    balance_cents: i64,
}

#[derive(sqlx::FromRow)]
struct PositionRow {
    symbol: String,
    quantity: f64,
}

#[async_trait]
impl WalletRepository for WalletRepositoryPg {
    #[instrument(skip(self))]
    async fn find(&self, user_id: &UserId) -> Result<Wallet, BusinessError> {
        let row: WalletRow = sqlx::query_as(
            "SELECT balance_cents FROM wallets WHERE user_id = $1",
        )
        .bind(user_id.0)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        let position_rows: Vec<PositionRow> = sqlx::query_as(
            "SELECT symbol, quantity FROM positions WHERE user_id = $1",
        )
        .bind(user_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        let mut positions = HashMap::new();
        for p in position_rows {
            let asset_type = symbol_to_asset_type(&p.symbol)?;
            positions.insert(p.symbol.clone(), Position { asset_type, quantity: p.quantity });
        }

        Ok(Wallet {
            user_id: user_id.clone(),
            balance_cents: row.balance_cents as u64,
            positions,
        })
    }

    #[instrument(skip(self))]
    async fn save(&self, wallet: &Wallet) -> Result<(), BusinessError> {
        sqlx::query(
            "UPDATE wallets SET balance_cents = $1 WHERE user_id = $2",
        )
        .bind(wallet.balance_cents as i64)
        .bind(wallet.user_id.0)
        .execute(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        for (symbol, position) in &wallet.positions {
            if position.quantity <= 0.0 {
                sqlx::query(
                    "DELETE FROM positions WHERE user_id = $1 AND symbol = $2",
                )
                .bind(wallet.user_id.0)
                .bind(symbol)
                .execute(&self.pool)
                .await
                .map_err(|_| BusinessError::UserNotFound)?;
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO positions (user_id, symbol, quantity)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (user_id, symbol)
                    DO UPDATE SET quantity = EXCLUDED.quantity
                    "#,
                )
                .bind(wallet.user_id.0)
                .bind(symbol)
                .bind(position.quantity)
                .execute(&self.pool)
                .await
                .map_err(|_| BusinessError::UserNotFound)?;
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn order_history(&self, user_id: &UserId) -> Result<Vec<Order>, BusinessError> {
        #[derive(sqlx::FromRow)]
        struct OrderRow {
            id: uuid::Uuid,
            order_type: String,
            symbol: String,
            quantity: f64,
            price_cents: i64,
            created_at: chrono::DateTime<chrono::Utc>,
        }

        let rows: Vec<OrderRow> = sqlx::query_as(
            r#"SELECT id, order_type, symbol, quantity, price_cents, created_at
               FROM orders WHERE user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(user_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        rows.into_iter()
            .map(|r| {
                let asset = symbol_to_asset_type(&r.symbol)?;
                let order_type = if r.order_type == "Buy" {
                    core_business::domain::order::OrderType::Buy
                } else {
                    core_business::domain::order::OrderType::Sell
                };
                Ok(Order {
                    id: core_business::domain::order::OrderId(r.id),
                    user_id: user_id.clone(),
                    order_type,
                    asset,
                    quantity: r.quantity,
                    price_cents: r.price_cents as u64,
                    created_at: r.created_at,
                })
            })
            .collect()
    }

    #[instrument(skip(self))]
    async fn record_order(&self, order: &Order) -> Result<(), BusinessError> {
        sqlx::query(
            r#"
            INSERT INTO orders (id, user_id, order_type, symbol, quantity, price_cents, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(order.id.0)
        .bind(order.user_id.0)
        .bind(format!("{:?}", order.order_type))
        .bind(order.asset.symbol())
        .bind(order.quantity)
        .bind(order.price_cents as i64)
        .bind(order.created_at)
        .execute(&self.pool)
        .await
        .map_err(|_| BusinessError::UserNotFound)?;

        Ok(())
    }
}

fn symbol_to_asset_type(symbol: &str) -> Result<AssetType, BusinessError> {
    match symbol {
        "BTC" => Ok(AssetType::Bitcoin),
        "ETH" => Ok(AssetType::Ethereum),
        "SOL" => Ok(AssetType::Solana),
        "USD" => Ok(AssetType::Dollar),
        other => Err(BusinessError::AssetNotFound(other.to_string())),
    }
}
