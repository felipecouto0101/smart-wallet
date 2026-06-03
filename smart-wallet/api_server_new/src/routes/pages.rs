#![forbid(unsafe_code)]

use std::sync::Arc;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{Redirect, Response},
};
use secrecy::SecretString;
use serde::Deserialize;

use core_business::domain::user::CreateUserDto;
use core_business::domain::order::OrderType;
use infra_data::repositories::user::verify_password;
use crate::auth::{generate_token, validate_token, JWT_COOKIE};
use crate::state::AppState;

// ── Template structs ────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPage {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterPage {
    pub error: Option<String>,
}

pub struct AssetView {
    pub symbol: String,
    pub name: String,
    pub price_display: String,
    pub price_cents: u64,
}

#[derive(Template)]
#[template(path = "assets.html")]
pub struct AssetsPage {
    pub assets: Vec<AssetView>,
}

pub struct PositionView {
    pub symbol: String,
    pub quantity: f64,
    pub price_cents: u64,
    pub price_display: String,
    pub total_display: String,
}

pub struct OrderView {
    pub is_buy: bool,
    pub symbol: String,
    pub quantity: f64,
    pub unit_price_display: String,
    pub total_display: String,
}

#[derive(Template)]
#[template(path = "wallet.html")]
pub struct WalletPage {
    pub balance_display: String,
    pub positions: Vec<PositionView>,
    pub orders: Vec<OrderView>,
    pub message: Option<String>,
    pub error: Option<String>,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn cents_to_display(cents: u64) -> String {
    let reais = cents / 100;
    let centavos = cents % 100;
    let reais_str = reais.to_string();
    let with_sep = reais_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(".");
    format!("{},{:02}", with_sep, centavos)
}

fn total_to_display(unit_price_cents: u64, quantity: f64) -> String {
    let total_cents = (unit_price_cents as f64 * quantity).round() as u64;
    cents_to_display(total_cents)
}

fn get_user_id(headers: &HeaderMap, secret: &str) -> Option<core_business::UserId> {
    let cookies = headers.get("cookie")?.to_str().ok()?;
    let token = cookies
        .split(';')
        .map(str::trim)
        .find(|c| c.starts_with(JWT_COOKIE))?
        .split_once('=')?
        .1;
    let claims = validate_token(token, secret).ok()?;
    let uuid = uuid::Uuid::parse_str(&claims.sub).ok()?;
    Some(core_business::UserId(uuid))
}

fn set_jwt_cookie(token: &str) -> axum::http::HeaderValue {
    format!("{}={}; HttpOnly; Path=/; SameSite=Strict", JWT_COOKIE, token)
        .parse()
        .unwrap()
}

// ── Page handlers ────────────────────────────────────────────────────────────

pub async fn get_login() -> impl IntoResponse {
    LoginPage { error: None }
}

pub async fn get_register() -> impl IntoResponse {
    RegisterPage { error: None }
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
}

pub async fn post_login(
    State(state): State<Arc<AppState>>,
    Form(form): Form<LoginForm>,
) -> Response {
    let user = match state.user_repo.find_by_email(&form.email).await {
        Ok(u) => u,
        Err(_) => return LoginPage { error: Some("Invalid credentials".into()) }.into_response(),
    };

    if !verify_password(&SecretString::from(form.password), &user.password_hash) {
        return LoginPage { error: Some("Invalid credentials".into()) }.into_response();
    }

    let token = match generate_token(&user.id.0, &state.jwt_secret) {
        Ok(t) => t,
        Err(_) => return LoginPage { error: Some("Internal error".into()) }.into_response(),
    };

    let mut response = Redirect::to("/wallet").into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        set_jwt_cookie(&token),
    );
    response
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub name: String,
    pub email: String,
    pub password: String,
}

pub async fn post_register(
    State(state): State<Arc<AppState>>,
    Form(form): Form<RegisterForm>,
) -> Response {
    let dto = CreateUserDto {
        name: form.name,
        email: form.email,
        password: SecretString::from(form.password),
    };

    match state.user_repo.create(dto).await {
        Ok(_) => Redirect::to("/login").into_response(),
        Err(e) => RegisterPage { error: Some(e.to_string()) }.into_response(),
    }
}

pub async fn get_assets(State(state): State<Arc<AppState>>) -> Response {
    match state.asset_repo.list_all().await {
        Ok(list) => AssetsPage {
            assets: list.into_iter().map(|a| AssetView {
                symbol: a.asset_type.symbol().to_string(),
                name: a.asset_type.name().to_string(),
                price_display: cents_to_display(a.price_cents),
                price_cents: a.price_cents,
            }).collect(),
        }.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load assets").into_response(),
    }
}

pub async fn get_wallet(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return Redirect::to("/login").into_response();
    };

    let wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(_) => return Redirect::to("/login").into_response(),
    };

    let orders = state.wallet_repo.order_history(&id).await.unwrap_or_default();
    let asset_prices = state.asset_repo.list_all().await.unwrap_or_default();

    WalletPage {
        balance_display: cents_to_display(wallet.balance_cents),
        positions: wallet.positions.values().map(|p| {
            let price_cents = asset_prices.iter()
                .find(|a| a.asset_type == p.asset_type)
                .map(|a| a.price_cents)
                .unwrap_or(0);
            PositionView {
                symbol: p.asset_type.symbol().to_string(),
                quantity: p.quantity,
                price_cents,
                price_display: cents_to_display(price_cents),
                total_display: total_to_display(price_cents, p.quantity),
            }
        }).collect(),
        orders: orders.iter().map(|o| OrderView {
            is_buy: o.order_type == OrderType::Buy,
            symbol: o.asset.symbol().to_string(),
            quantity: o.quantity,
            unit_price_display: cents_to_display(o.price_cents),
            total_display: total_to_display(o.price_cents, o.quantity),
        }).collect(),
        message: params.get("msg").cloned(),
        error: params.get("err").map(|e| e.replace('+', " ")),
    }.into_response()
}

#[derive(Deserialize)]
pub struct TradeForm {
    pub symbol: String,
    pub quantity: String,
}

fn parse_quantity(s: &str) -> Result<f64, String> {
    let normalized = s.trim().replace(',', ".");
    let q: f64 = normalized.parse().map_err(|_| "Invalid quantity".to_string())?;
    if q <= 0.0 || !q.is_finite() {
        return Err("Quantity must be greater than zero".to_string());
    }
    Ok(q)
}

pub async fn post_buy(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return Redirect::to("/login").into_response();
    };

    let quantity = match parse_quantity(&form.quantity) {
        Ok(q) => q,
        Err(e) => return wallet_page_with_error(&state, &id, e).await,
    };

    let asset = match state.asset_repo.find_price(&form.symbol).await {
        Ok(a) => a,
        Err(e) => return wallet_page_with_error(&state, &id, e.to_string()).await,
    };

    let mut wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => return wallet_page_with_error(&state, &id, e.to_string()).await,
    };

    match wallet.buy(&asset.asset_type, quantity, asset.price_cents) {
        Ok(()) => {
            let order = core_business::domain::order::Order::new(
                id.clone(), OrderType::Buy, asset.asset_type, quantity, asset.price_cents,
            );
            let _ = state.wallet_repo.save(&wallet).await;
            let _ = state.wallet_repo.record_order(&order).await;
            Redirect::to("/wallet?msg=purchase+successful").into_response()
        }
        Err(e) => {
            let msg = e.to_string().replace(' ', "+");
            Redirect::to(&format!("/wallet?err={}", msg)).into_response()
        }
    }
}

pub async fn post_sell(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<TradeForm>,
) -> Response {
    let Some(id) = get_user_id(&headers, &state.jwt_secret) else {
        return Redirect::to("/login").into_response();
    };

    let quantity = match parse_quantity(&form.quantity) {
        Ok(q) => q,
        Err(e) => return wallet_page_with_error(&state, &id, e).await,
    };

    let asset = match state.asset_repo.find_price(&form.symbol).await {
        Ok(a) => a,
        Err(e) => return wallet_page_with_error(&state, &id, e.to_string()).await,
    };

    let mut wallet = match state.wallet_repo.find(&id).await {
        Ok(w) => w,
        Err(e) => return wallet_page_with_error(&state, &id, e.to_string()).await,
    };

    match wallet.sell(&asset.asset_type, quantity, asset.price_cents) {
        Ok(()) => {
            let order = core_business::domain::order::Order::new(
                id.clone(), OrderType::Sell, asset.asset_type, quantity, asset.price_cents,
            );
            let _ = state.wallet_repo.save(&wallet).await;
            let _ = state.wallet_repo.record_order(&order).await;
            Redirect::to("/wallet?msg=sale+successful").into_response()
        }
        Err(e) => {
            let msg = e.to_string().replace(' ', "+");
            Redirect::to(&format!("/wallet?err={}", msg)).into_response()
        }
    }
}

pub async fn get_logout() -> Response {
    let clear = format!("{}=; HttpOnly; Path=/; Max-Age=0", JWT_COOKIE);
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        clear.parse().unwrap(),
    );
    response
}

async fn wallet_page_with_error(
    state: &Arc<AppState>,
    id: &core_business::UserId,
    error: String,
) -> Response {
    let wallet = state.wallet_repo.find(id).await.ok();
    let orders = state.wallet_repo.order_history(id).await.unwrap_or_default();
    let asset_prices = state.asset_repo.list_all().await.unwrap_or_default();

    WalletPage {
        balance_display: cents_to_display(wallet.as_ref().map(|w| w.balance_cents).unwrap_or(0)),
        positions: wallet.map(|w| w.positions.values().map(|p| {
            let price_cents = asset_prices.iter()
                .find(|a| a.asset_type == p.asset_type)
                .map(|a| a.price_cents)
                .unwrap_or(0);
            PositionView {
                symbol: p.asset_type.symbol().to_string(),
                quantity: p.quantity,
                price_cents,
                price_display: cents_to_display(price_cents),
                total_display: total_to_display(price_cents, p.quantity),
            }
        }).collect()).unwrap_or_default(),
        orders: orders.iter().map(|o| OrderView {
            is_buy: o.order_type == OrderType::Buy,
            symbol: o.asset.symbol().to_string(),
            quantity: o.quantity,
            unit_price_display: cents_to_display(o.price_cents),
            total_display: total_to_display(o.price_cents, o.quantity),
        }).collect(),
        message: None,
        error: Some(error),
    }.into_response()
}
