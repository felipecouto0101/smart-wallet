#![forbid(unsafe_code)]

pub mod db;
pub mod repositories;

pub use db::connect;
