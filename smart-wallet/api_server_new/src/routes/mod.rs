#![forbid(unsafe_code)]

pub mod auth;
pub mod wallet;
pub mod assets;
pub mod pages;

use std::sync::Arc;
use axum::{Router, routing::{get, post}};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Pages (HTML)
        .route("/",             get(pages::get_login))
        .route("/login",        get(pages::get_login).post(pages::post_login))
        .route("/register",     get(pages::get_register).post(pages::post_register))
        .route("/logout",       get(pages::get_logout))
        .route("/assets",       get(pages::get_assets))
        .route("/wallet",       get(pages::get_wallet))
        .route("/wallet/buy",   post(pages::post_buy))
        .route("/wallet/sell",  post(pages::post_sell))
        // JSON API
        .route("/api/assets",          get(assets::list))
        .route("/api/register",        post(auth::register))
        .route("/api/login",           post(auth::login))
        .route("/api/logout",          post(auth::logout))
        .route("/api/wallet",          get(wallet::view))
        .route("/api/wallet/buy",      post(wallet::buy))
        .route("/api/wallet/sell",     post(wallet::sell))
        .route("/api/wallet/history",  get(wallet::history))
        // Global tracing middleware
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
