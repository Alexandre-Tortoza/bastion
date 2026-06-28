//! Bearer token auth middleware.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use axum::Json;

use crate::state::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match token {
        Some(t) if t == state.config.api_token => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized", "code": "UNAUTHORIZED" })),
        )
            .into_response(),
    }
}
