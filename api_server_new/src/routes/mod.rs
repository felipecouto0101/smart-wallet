#![forbid(unsafe_code)]

pub mod auth;
pub mod wallet;
pub mod assets;
pub mod pages;
pub mod saldo_real;

use std::sync::Arc;
use axum::{Router, middleware, routing::{get, post}};
use tower_http::trace::TraceLayer;

use crate::auth::middleware::require_auth;
use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Routes accessible without authentication
    let public = Router::new()
        .route("/",          get(pages::get_login))
        .route("/login",     get(pages::get_login).post(pages::post_login))
        .route("/register",  get(pages::get_register).post(pages::post_register))
        .route("/api/register", post(auth::register))
        .route("/api/login",    post(auth::login));

    // Routes that require a valid JWT cookie
    let protected = Router::new()
        .route("/logout",              get(pages::get_logout))
        .route("/assets",              get(pages::get_assets))
        .route("/wallet",              get(pages::get_wallet))
        .route("/wallet/buy",          post(pages::post_buy))
        .route("/wallet/sell",         post(pages::post_sell))
        .route("/wallet/simulate",     post(pages::post_simulate))
        .route("/saldo-real",           get(saldo_real::get_saldo_real))
        .route("/api/assets",          get(assets::list))
        .route("/api/logout",          post(auth::logout))
        .route("/api/wallet",          get(wallet::view))
        .route("/api/wallet/buy",      post(wallet::buy))
        .route("/api/wallet/sell",     post(wallet::sell))
        .route("/api/wallet/history",  get(wallet::history))
        .route("/api/saldo-real",      get(saldo_real::api_analyze))
        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
