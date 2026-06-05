#![forbid(unsafe_code)]

pub mod db;
pub mod price_provider;
pub mod repositories;

pub use db::connect;
pub use price_provider::LivePriceProvider;
pub use repositories::asset::update_prices;
pub use repositories::FinancialProfileRepositoryPg;
pub use repositories::{generate_for_kind, reproduce_for_kind, ProfileKind};
