#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::domain::asset::AssetType;
use crate::domain::user::UserId;

/// Newtype for order ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderType {
    Buy,
    Sell,
}

/// Immutable record of an executed order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub user_id: UserId,
    pub order_type: OrderType,
    pub asset: AssetType,
    pub quantity: f64,
    pub price_cents: u64,
    pub created_at: DateTime<Utc>,
}

impl Order {
    pub fn new(
        user_id: UserId,
        order_type: OrderType,
        asset: AssetType,
        quantity: f64,
        price_cents: u64,
    ) -> Self {
        Self {
            id: OrderId::new(),
            user_id,
            order_type,
            asset,
            quantity,
            price_cents,
            created_at: Utc::now(),
        }
    }
}
