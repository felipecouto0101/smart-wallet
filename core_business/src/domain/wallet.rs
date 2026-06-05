#![forbid(unsafe_code)]

use crate::domain::asset::AssetType;
use crate::domain::user::UserId;
use crate::errors::BusinessError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A position in a specific asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub asset_type: AssetType,
    /// Fractional quantity (e.g. 0.001 BTC).
    pub quantity: f64,
    /// Average purchase cost in cents (BRL) per unit.
    pub avg_cost_cents: f64,
}

/// A user's investment wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub user_id: UserId,
    /// Available balance in cents (BRL).
    pub balance_cents: u64,
    pub positions: HashMap<String, Position>,
}

impl Wallet {
    pub fn new(user_id: UserId, initial_balance: u64) -> Self {
        Self {
            user_id,
            balance_cents: initial_balance,
            positions: HashMap::new(),
        }
    }

    /// Executes a buy order for an asset, supporting fractional quantities.
    ///
    /// # Example
    /// ```
    /// use core_business::{Wallet, AssetType};
    /// use core_business::domain::user::UserId;
    ///
    /// let id = UserId::new();
    /// let mut wallet = Wallet::new(id, 100_000_000);
    /// let result = wallet.buy(&AssetType::Bitcoin, 0.001, 35_000_000);
    /// assert!(result.is_ok());
    /// ```
    pub fn buy(
        &mut self,
        asset_type: &AssetType,
        quantity: f64,
        unit_price_cents: u64,
    ) -> Result<(), BusinessError> {
        if quantity <= 0.0 || !quantity.is_finite() {
            return Err(BusinessError::InvalidQuantity);
        }

        let total_cost = (unit_price_cents as f64 * quantity).round() as u64;

        if self.balance_cents < total_cost {
            return Err(BusinessError::InsufficientBalance);
        }

        self.balance_cents -= total_cost;

        let position = self
            .positions
            .entry(asset_type.symbol().to_string())
            .or_insert(Position {
                asset_type: asset_type.clone(),
                quantity: 0.0,
                avg_cost_cents: 0.0,
            });

        // weighted average cost: (old_qty * old_avg + new_qty * price) / total_qty
        let new_qty = position.quantity + quantity;
        position.avg_cost_cents = (position.quantity * position.avg_cost_cents
            + quantity * unit_price_cents as f64)
            / new_qty;
        position.quantity = new_qty;
        Ok(())
    }

    /// Executes a sell order for an asset, supporting fractional quantities.
    pub fn sell(
        &mut self,
        asset_type: &AssetType,
        quantity: f64,
        unit_price_cents: u64,
    ) -> Result<(), BusinessError> {
        if quantity <= 0.0 || !quantity.is_finite() {
            return Err(BusinessError::InvalidQuantity);
        }

        let position = self
            .positions
            .get_mut(asset_type.symbol())
            .ok_or(BusinessError::InsufficientPosition)?;

        if position.quantity < quantity {
            return Err(BusinessError::InsufficientPosition);
        }

        let revenue = (unit_price_cents as f64 * quantity).round() as u64;

        position.quantity -= quantity;
        self.balance_cents += revenue;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::{assert_err, assert_ok};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    fn test_wallet() -> Wallet {
        Wallet::new(UserId::new(), 100_000)
    }

    #[test]
    fn valid_buy_debits_balance() {
        let mut w = test_wallet();
        assert_ok!(w.buy(&AssetType::Dollar, 10.0, 500));
        assert_eq!(w.balance_cents, 95_000);
    }

    #[test]
    fn fractional_buy_debits_correctly() {
        // 0.001 BTC at 35_000_000 cents (R$350.000,00) = 35_000 cents (R$350,00)
        let mut w = Wallet::new(UserId::new(), 100_000_000);
        assert_ok!(w.buy(&AssetType::Bitcoin, 0.001, 35_000_000));
        assert_eq!(w.balance_cents, 99_965_000);
    }

    #[test]
    fn buy_with_insufficient_balance_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.buy(&AssetType::Bitcoin, 1.0, 200_000));
    }

    #[test]
    fn buy_zero_quantity_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.buy(&AssetType::Solana, 0.0, 100));
    }

    #[test]
    fn buy_negative_quantity_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.buy(&AssetType::Solana, -1.0, 100));
    }

    #[test]
    fn valid_sell_credits_balance() {
        let mut w = test_wallet();
        assert_ok!(w.buy(&AssetType::Dollar, 10.0, 500));
        assert_ok!(w.sell(&AssetType::Dollar, 5.0, 500));
        assert_eq!(w.balance_cents, 97_500);
    }

    #[test]
    fn sell_without_position_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.sell(&AssetType::Bitcoin, 1.0, 100));
    }

    #[test]
    fn sell_above_position_returns_error() {
        let mut w = test_wallet();
        assert_ok!(w.buy(&AssetType::Dollar, 5.0, 100));
        assert_err!(w.sell(&AssetType::Dollar, 10.0, 100));
    }

    #[test]
    fn buy_nan_quantity_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.buy(&AssetType::Dollar, f64::NAN, 100));
    }

    #[test]
    fn buy_infinite_quantity_returns_error() {
        let mut w = test_wallet();
        assert_err!(w.buy(&AssetType::Dollar, f64::INFINITY, 100));
    }

    // ── Property-based tests ─────────────────────────────────────────────────

    proptest! {
        /// Balance after a successful buy must always decrease by exactly cost.
        #[test]
        fn prop_buy_decreases_balance_by_exact_cost(
            initial in 10_000_000u64..1_000_000_000u64,
            qty     in 1u32..1_000u32,
            price   in 1u64..100_000u64,
        ) {
            let qty_f = qty as f64;
            let cost  = (price as f64 * qty_f).round() as u64;
            if cost <= initial {
                let mut w = Wallet::new(UserId::new(), initial);
                assert_ok!(w.buy(&AssetType::Dollar, qty_f, price));
                prop_assert_eq!(w.balance_cents, initial - cost);
            }
        }

        /// Buy followed by full sell must restore balance to original value.
        #[test]
        fn prop_buy_then_sell_same_price_restores_balance(
            initial in 10_000_000u64..1_000_000_000u64,
            qty     in 1u32..1_000u32,
            price   in 1u64..100_000u64,
        ) {
            let qty_f = qty as f64;
            let cost  = (price as f64 * qty_f).round() as u64;
            if cost <= initial {
                let mut w = Wallet::new(UserId::new(), initial);
                assert_ok!(w.buy(&AssetType::Dollar, qty_f, price));
                assert_ok!(w.sell(&AssetType::Dollar, qty_f, price));
                prop_assert_eq!(w.balance_cents, initial);
            }
        }

        /// Selling more than owned must always fail.
        #[test]
        fn prop_sell_excess_always_fails(
            initial in 10_000_000u64..1_000_000_000u64,
            qty     in 1u32..500u32,
            price   in 1u64..100_000u64,
        ) {
            let qty_f  = qty as f64;
            let cost   = (price as f64 * qty_f).round() as u64;
            if cost <= initial {
                let mut w = Wallet::new(UserId::new(), initial);
                assert_ok!(w.buy(&AssetType::Dollar, qty_f, price));
                // try to sell double
                let result = w.sell(&AssetType::Dollar, qty_f * 2.0, price);
                prop_assert!(result.is_err());
            }
        }
    }
}
