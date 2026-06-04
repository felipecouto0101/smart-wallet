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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn order_id_generates_unique_values() {
        assert_ne!(OrderId::new(), OrderId::new());
    }

    #[test]
    fn order_id_default_is_unique() {
        assert_ne!(OrderId::default(), OrderId::default());
    }

    #[test]
    fn order_new_preserves_fields() {
        let uid = UserId::new();
        let order = Order::new(uid.clone(), OrderType::Buy, AssetType::Bitcoin, 0.5, 35_000_000);
        assert_eq!(order.user_id, uid);
        assert_eq!(order.order_type, OrderType::Buy);
        assert_eq!(order.asset, AssetType::Bitcoin);
        assert_eq!(order.quantity, 0.5);
        assert_eq!(order.price_cents, 35_000_000);
    }

    #[test]
    fn sell_order_type_is_preserved() {
        let order = Order::new(UserId::new(), OrderType::Sell, AssetType::Ethereum, 1.0, 10_000_000);
        assert_eq!(order.order_type, OrderType::Sell);
        assert_eq!(order.asset, AssetType::Ethereum);
    }

    #[test]
    fn order_created_at_is_recent() {
        let before = Utc::now();
        let order = Order::new(UserId::new(), OrderType::Buy, AssetType::Dollar, 10.0, 500);
        let after = Utc::now();
        assert!(order.created_at >= before && order.created_at <= after);
    }
}
