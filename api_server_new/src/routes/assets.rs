#![forbid(unsafe_code)]

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use std::sync::Arc;

use crate::state::AppState;

pub async fn list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.asset_repo.list_all().await {
        Ok(assets) => (StatusCode::OK, Json(serde_json::json!({"assets": assets}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
