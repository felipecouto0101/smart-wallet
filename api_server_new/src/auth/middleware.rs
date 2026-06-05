#![forbid(unsafe_code)]

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use std::sync::Arc;

use crate::auth::{validate_token, JWT_COOKIE};
use crate::state::AppState;

/// Axum middleware that enforces authentication on every route it wraps.
///
/// - HTML routes (`Accept: text/html` or no `Accept`) → redirect to `/login`
/// - API routes (path starts with `/api/`) → 401 JSON response
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let is_api = req.uri().path().starts_with("/api/");

    let token = req
        .headers()
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(str::trim)
                .find(|c| c.starts_with(JWT_COOKIE))
                .and_then(|c| c.split_once('='))
                .map(|(_, t)| t.to_owned())
        });

    let valid = token
        .as_deref()
        .map(|t| validate_token(t, &state.jwt_secret).is_ok())
        .unwrap_or(false);

    if valid {
        next.run(req).await
    } else if is_api {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "not authenticated"})),
        )
            .into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}
