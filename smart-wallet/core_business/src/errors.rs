#![forbid(unsafe_code)]

use thiserror::Error;

/// Business logic errors for the investment wallet.
#[derive(Debug, Error, PartialEq)]
pub enum BusinessError {
    #[error("Insufficient balance to complete the operation")]
    InsufficientBalance,

    #[error("Invalid quantity: must be greater than zero")]
    InvalidQuantity,

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Insufficient position to sell")]
    InsufficientPosition,

    #[error("A user with this email already exists")]
    DuplicateUser,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Failed to fetch live prices: {0}")]
    PriceFetchError(String),
}
