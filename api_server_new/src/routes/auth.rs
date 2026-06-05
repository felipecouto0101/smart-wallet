#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

use crate::{
    auth::{generate_token, JWT_COOKIE},
    state::AppState,
};
use core_business::domain::user::CreateUserDto;
use infra_data::repositories::user::verify_password;

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterPayload {
    #[validate(length(min = 2, max = 100))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginPayload {
    #[validate(email)]
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub name: String,
    pub email: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterPayload>,
) -> impl IntoResponse {
    if payload.validate().is_err() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "invalid input"})),
        )
            .into_response();
    }

    let dto = CreateUserDto {
        name: payload.name,
        email: payload.email,
        password: SecretString::from(payload.password),
    };

    match state.user_repo.create(dto).await {
        Ok(user) => (
            StatusCode::CREATED,
            Json(UserResponse {
                id: user.id.0.to_string(),
                name: user.name,
                email: user.email,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginPayload>,
) -> impl IntoResponse {
    if payload.validate().is_err() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "invalid input"})),
        )
            .into_response();
    }

    let user = match state.user_repo.find_by_email(&payload.email).await {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid credentials"})),
            )
                .into_response()
        }
    };

    if !verify_password(&SecretString::from(payload.password), &user.password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid credentials"})),
        )
            .into_response();
    }

    match generate_token(&user.id.0, &state.jwt_secret) {
        Ok(token) => {
            let cookie = format!(
                "{}={}; HttpOnly; Path=/; SameSite=Strict",
                JWT_COOKIE, token
            );
            let mut headers = HeaderMap::new();
            headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
            (
                StatusCode::OK,
                headers,
                Json(serde_json::json!({"message": "login successful"})),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal error"})),
        )
            .into_response(),
    }
}

pub async fn logout() -> impl IntoResponse {
    let cookie = format!("{}=; HttpOnly; Path=/; Max-Age=0", JWT_COOKIE);
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
    (
        StatusCode::OK,
        headers,
        Json(serde_json::json!({"message": "logout successful"})),
    )
}
