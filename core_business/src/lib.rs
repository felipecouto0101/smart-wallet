#![forbid(unsafe_code)]

pub mod domain;
pub mod errors;
pub mod ports;

pub use domain::asset::{Asset, AssetType};
pub use domain::financial_profile::{
    analyze_purchase, fmt_brl, proactive_alerts, AlertLevel, FinancialProfile, PurchaseAnalysis,
    WalletAlert,
};
pub use domain::order::{Order, OrderType};
pub use domain::user::{User, UserId};
pub use domain::wallet::{Position, Wallet};
pub use errors::BusinessError;
pub use ports::{
    AssetRepository, FinancialProfileRepository, PriceProvider, UserRepository, WalletRepository,
};
