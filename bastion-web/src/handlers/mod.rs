pub mod config;
pub mod ingest;
pub mod review;
pub mod wiki;

use axum::{Json, response::IntoResponse};
use serde_json::json;

pub async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}
