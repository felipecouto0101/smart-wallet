#![forbid(unsafe_code)]

pub mod domain;
pub mod errors;
pub mod ports;

pub use domain::asset::{Asset, AssetType};
pub use domain::wallet::{Wallet, Position};
pub use domain::user::{UserId, User};
pub use domain::order::{Order, OrderType};
pub use errors::BusinessError;
pub use ports::{UserRepository, WalletRepository, AssetRepository, PriceProvider};
