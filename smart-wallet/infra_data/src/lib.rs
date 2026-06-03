#![forbid(unsafe_code)]

pub mod db;
pub mod repositories;
pub mod price_provider;

pub use db::connect;
pub use price_provider::LivePriceProvider;
pub use repositories::asset::update_prices;
