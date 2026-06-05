#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::{validate_token, JWT_COOKIE},
    state::AppState,
};
use core_business::{
    domain::order::{Order, OrderType},
    UserId,
};

fn extract_user_id(headers: &HeaderMap, secret: &str) -> Option<UserId> {
    let cookies = headers.get("cookie")?.to_str().ok()?;
    let token = cookies
        .split(';')
        .map(str::trim)
        .find(|c| c.starts_with(JWT_COOKIE))?
        .split_once('=')?
        .1;
    let claims = validate_token(token, secret).ok()?;
    let uuid = Uuid::parse_str(&claims.sub).ok()?;
    Some(UserId(uuid))
}

pub async fn view(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let Some(id) = extract_user_id(&headers, &state.jwt_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response();
    };

    match state.wallet_repo.find(&id).await {
        Ok(wallet) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "balance_cents": wallet.balance_cents,
                "positions": wallet.positions,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct OrderPayload {
    pub symbol: String,
    pub quantity: f64,
}

pub async fn buy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<OrderPayload>,
) -> impl IntoResponse {
    let Some(id) = extract_user_id(&headers, &state.jwt_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response();
    };

    let asset = match state.asset_repo.find_price(&payload.symbol).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let mut wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    match wallet.buy(&asset.asset_type, payload.quantity, asset.price_cents) {
        Ok(()) => {
            let order = Order::new(
                id.clone(),
                OrderType::Buy,
                asset.asset_type,
                payload.quantity,
                asset.price_cents,
            );
            let _ = state.wallet_repo.save(&wallet).await;
            let _ = state.wallet_repo.record_order(&order).await;
            (StatusCode::OK, Json(serde_json::json!({"message": "purchase successful", "balance_cents": wallet.balance_cents}))).into_response()
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn sell(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<OrderPayload>,
) -> impl IntoResponse {
    let Some(id) = extract_user_id(&headers, &state.jwt_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response();
    };

    let asset = match state.asset_repo.find_price(&payload.symbol).await {
        Ok(a) => a,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let mut wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    match wallet.sell(&asset.asset_type, payload.quantity, asset.price_cents) {
        Ok(()) => {
            let order = Order::new(
                id.clone(),
                OrderType::Sell,
                asset.asset_type,
                payload.quantity,
                asset.price_cents,
            );
            let _ = state.wallet_repo.save(&wallet).await;
            let _ = state.wallet_repo.record_order(&order).await;
            (StatusCode::OK, Json(serde_json::json!({"message": "sale successful", "balance_cents": wallet.balance_cents}))).into_response()
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn history(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let Some(id) = extract_user_id(&headers, &state.jwt_secret) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response();
    };

    match state.wallet_repo.order_history(&id).await {
        Ok(orders) => (StatusCode::OK, Json(serde_json::json!({"orders": orders}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
