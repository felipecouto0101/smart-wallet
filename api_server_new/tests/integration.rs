//! Integration (E2E) tests for the JSON API routes.
//!
//! All infrastructure traits are mocked via `mockall` — no real DB or network.
//! Each test spins up an in-process Axum router via `tower::ServiceExt::oneshot`.

#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

use core_business::{
    domain::{
        order::{Order, OrderType},
        user::{User, UserId},
    },
    ports::{MockAssetRepository, MockUserRepository, MockWalletRepository},
    Asset, AssetType, BusinessError, FinancialProfile, Wallet,
};

use api_server::routes::create_router;
use api_server::state::{AppState, MockFinancialProfileRepository};

// ── Helpers ──────────────────────────────────────────────────────────────────

const JWT_SECRET: &str = "test_secret_that_is_long_enough_!";

fn fixed_user_id() -> UserId {
    UserId(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
}

fn auth_cookie() -> String {
    let token = api_server::auth::generate_token(&fixed_user_id().0, JWT_SECRET).unwrap();
    format!("smart_wallet_token={token}")
}

fn base_profile() -> FinancialProfile {
    FinancialProfile {
        card_bill_cents: 10_000,
        card_bill_days: 20,
        pending_bills_cents: 5_000,
        estimated_variable_spend_cents: 5_000,
        monthly_savings_goal_cents: 10_000,
        days_until_salary: 15,
        monthly_income_cents: 500_000,
    }
}

fn make_state(
    user_repo: MockUserRepository,
    wallet_repo: MockWalletRepository,
    asset_repo: MockAssetRepository,
    profile_repo: MockFinancialProfileRepository,
) -> Arc<AppState> {
    Arc::new(AppState {
        user_repo: Arc::new(user_repo),
        wallet_repo: Arc::new(wallet_repo),
        asset_repo: Arc::new(asset_repo),
        profile_repo: Arc::new(profile_repo),
        jwt_secret: JWT_SECRET.to_string(),
    })
}

async fn send(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = router.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

// ── POST /api/register ────────────────────────────────────────────────────────

#[tokio::test]
async fn register_returns_201_on_success() {
    let mut user_repo = MockUserRepository::new();
    user_repo.expect_create().once().returning(|dto| {
        Box::pin(async move {
            Ok(User::new(
                UserId::new(),
                dto.name,
                dto.email,
                secrecy::SecretString::from("hash"),
            ))
        })
    });

    let state = make_state(
        user_repo,
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"name":"Alice","email":"alice@test.com","password":"secret123"}"#,
        ))
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["email"], "alice@test.com");
}

#[tokio::test]
async fn register_returns_422_on_invalid_email() {
    let state = make_state(
        MockUserRepository::new(),
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"name":"Alice","email":"not-an-email","password":"secret123"}"#,
        ))
        .unwrap();

    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn register_returns_409_on_duplicate() {
    let mut user_repo = MockUserRepository::new();
    user_repo
        .expect_create()
        .once()
        .returning(|_| Box::pin(async { Err(BusinessError::DuplicateUser) }));

    let state = make_state(
        user_repo,
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .method("POST")
        .uri("/api/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"name":"Alice","email":"alice@test.com","password":"secret123"}"#,
        ))
        .unwrap();

    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

// ── GET /api/assets ───────────────────────────────────────────────────────────

#[tokio::test]
async fn list_assets_returns_401_without_auth() {
    let state = make_state(
        MockUserRepository::new(),
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .uri("/api/assets")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_assets_returns_200_with_assets() {
    let mut asset_repo = MockAssetRepository::new();
    asset_repo.expect_list_all().once().returning(|| {
        Box::pin(async {
            Ok(vec![
                Asset::new(AssetType::Bitcoin, 35_000_000),
                Asset::new(AssetType::Dollar, 550),
            ])
        })
    });

    let state = make_state(
        MockUserRepository::new(),
        MockWalletRepository::new(),
        asset_repo,
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .uri("/api/assets")
        .header("cookie", auth_cookie())
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(create_router(state), req).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["assets"].as_array().unwrap().len(), 2);
}

// ── GET /api/wallet ───────────────────────────────────────────────────────────

#[tokio::test]
async fn get_wallet_returns_401_without_auth() {
    let state = make_state(
        MockUserRepository::new(),
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .uri("/api/wallet")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_wallet_returns_200_with_valid_token() {
    let uid = fixed_user_id();
    let mut wallet_repo = MockWalletRepository::new();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid = uid.clone();
        Box::pin(async move { Ok(Wallet::new(uid, 500_000)) })
    });

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .uri("/api/wallet")
        .header("cookie", auth_cookie())
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["balance_cents"], 500_000);
}

// ── POST /api/wallet/buy ──────────────────────────────────────────────────────

#[tokio::test]
async fn buy_returns_200_on_valid_order() {
    let uid = fixed_user_id();

    let mut asset_repo = MockAssetRepository::new();
    asset_repo
        .expect_find_price()
        .once()
        .returning(|_| Box::pin(async { Ok(Asset::new(AssetType::Dollar, 550)) }));

    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid2 = uid2.clone();
        Box::pin(async move { Ok(Wallet::new(uid2, 100_000_000)) })
    });
    wallet_repo
        .expect_save()
        .once()
        .returning(|_| Box::pin(async { Ok(()) }));
    wallet_repo
        .expect_record_order()
        .once()
        .returning(|_| Box::pin(async { Ok(()) }));

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        asset_repo,
        MockFinancialProfileRepository::new(),
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .method("POST")
        .uri("/api/wallet/buy")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(r#"{"symbol":"USD","quantity":10.0}"#))
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "purchase successful");
}

#[tokio::test]
async fn buy_returns_422_on_insufficient_balance() {
    let uid = fixed_user_id();

    let mut asset_repo = MockAssetRepository::new();
    asset_repo
        .expect_find_price()
        .once()
        .returning(|_| Box::pin(async { Ok(Asset::new(AssetType::Bitcoin, 35_000_000)) }));

    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid2 = uid2.clone();
        Box::pin(async move { Ok(Wallet::new(uid2, 0)) })
    });

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        asset_repo,
        MockFinancialProfileRepository::new(),
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .method("POST")
        .uri("/api/wallet/buy")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(r#"{"symbol":"BTC","quantity":1.0}"#))
        .unwrap();

    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ── POST /api/wallet/sell ─────────────────────────────────────────────────────

#[tokio::test]
async fn sell_returns_422_when_no_position() {
    let uid = fixed_user_id();

    let mut asset_repo = MockAssetRepository::new();
    asset_repo
        .expect_find_price()
        .once()
        .returning(|_| Box::pin(async { Ok(Asset::new(AssetType::Bitcoin, 35_000_000)) }));

    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid2 = uid2.clone();
        Box::pin(async move { Ok(Wallet::new(uid2, 100_000_000)) })
    });

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        asset_repo,
        MockFinancialProfileRepository::new(),
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .method("POST")
        .uri("/api/wallet/sell")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(r#"{"symbol":"BTC","quantity":1.0}"#))
        .unwrap();

    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ── GET /api/wallet/history ───────────────────────────────────────────────────

#[tokio::test]
async fn history_returns_empty_list() {
    let mut wallet_repo = MockWalletRepository::new();
    wallet_repo
        .expect_order_history()
        .once()
        .returning(|_| Box::pin(async { Ok(vec![]) }));

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .uri("/api/wallet/history")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["orders"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn history_returns_recorded_orders() {
    let uid = fixed_user_id();
    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo
        .expect_order_history()
        .once()
        .returning(move |_| {
            let uid2 = uid2.clone();
            Box::pin(async move {
                Ok(vec![Order::new(
                    uid2,
                    OrderType::Buy,
                    AssetType::Dollar,
                    5.0,
                    550,
                )])
            })
        });

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .uri("/api/wallet/history")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["orders"].as_array().unwrap().len(), 1);
}

// ── GET /api/saldo-real ───────────────────────────────────────────────────────

#[tokio::test]
async fn saldo_real_returns_401_without_auth() {
    let state = make_state(
        MockUserRepository::new(),
        MockWalletRepository::new(),
        MockAssetRepository::new(),
        MockFinancialProfileRepository::new(),
    );
    let req = Request::builder()
        .uri("/api/saldo-real")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn saldo_real_returns_proactive_alerts_with_valid_token() {
    let uid = fixed_user_id();

    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid2 = uid2.clone();
        Box::pin(async move { Ok(Wallet::new(uid2, 50_000)) })
    });

    let mut profile_repo = MockFinancialProfileRepository::new();
    profile_repo
        .expect_get_or_generate()
        .once()
        .returning(|_| Box::pin(async { Ok(base_profile()) }));

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        MockAssetRepository::new(),
        profile_repo,
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .uri("/api/saldo-real")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["proactive_alerts"].as_array().unwrap().is_empty());
    assert_eq!(body["balance_cents"], 50_000);
}

#[tokio::test]
async fn saldo_real_returns_purchase_analysis_when_amount_provided() {
    let uid = fixed_user_id();

    let mut wallet_repo = MockWalletRepository::new();
    let uid2 = uid.clone();
    wallet_repo.expect_find().once().returning(move |_| {
        let uid2 = uid2.clone();
        Box::pin(async move { Ok(Wallet::new(uid2, 50_000)) })
    });

    let mut profile_repo = MockFinancialProfileRepository::new();
    profile_repo
        .expect_get_or_generate()
        .once()
        .returning(|_| Box::pin(async { Ok(base_profile()) }));

    let state = make_state(
        MockUserRepository::new(),
        wallet_repo,
        MockAssetRepository::new(),
        profile_repo,
    );
    let cookie = auth_cookie();
    let req = Request::builder()
        .uri("/api/saldo-real?amount=100.00")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(create_router(state), req).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["analysis"].is_null());
}
