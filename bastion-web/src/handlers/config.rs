use axum::{Json, extract::State};
use bastion_llm::{build_embedder, build_llm_provider};
use serde::Deserialize;
use serde_json::json;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ConfigPayload {
    pub llm_provider: Option<String>,
    pub llm_key: Option<String>,
    pub llm_model: Option<String>,
    pub embed_provider: Option<String>,
    pub embed_key: Option<String>,
    pub embed_model: Option<String>,
}

pub async fn set_config(
    State(state): State<AppState>,
    Json(payload): Json<ConfigPayload>,
) -> WebResult<Json<serde_json::Value>> {
    let cfg = &state.config;
    let lk = payload.llm_key.as_deref();
    let ek = payload.embed_key.as_deref();

    // Build LLM: key from payload takes priority over env var for each provider.
    let llm = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build_llm_provider(
            payload.llm_provider.as_deref(),
            payload.llm_model.as_deref(),
            lk.or(cfg.openai_key.as_deref()),
            lk.or(cfg.anthropic_key.as_deref()),
            lk.or(cfg.openrouter_key.as_deref()),
            lk.or(cfg.gemini_key.as_deref()),
        )
    }))
    .map_err(|_| WebError::BadRequest("invalid LLM provider or missing API key".into()))?;

    let embedder = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build_embedder(
            payload.embed_provider.as_deref(),
            payload.embed_model.as_deref(),
            ek.or(cfg.openai_key.as_deref()),
            ek.or(cfg.voyage_key.as_deref()),
            ek.or(cfg.gemini_key.as_deref()),
        )
    }))
    .map_err(|_| WebError::BadRequest("invalid embed provider or missing API key".into()))?;

    let mut overrides = state.overrides.write().unwrap();
    if let Some(l) = llm {
        overrides.llm = Some(l);
    }
    if let Some(e) = embedder {
        overrides.embedder = Some(e);
    }

    Ok(Json(json!({ "ok": true })))
}

pub async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    let ov = state.overrides.read().unwrap();
    let llm_configured = ov.llm.is_some() || state.llm.is_some();
    let embed_configured = ov.embedder.is_some() || state.embedder.is_some();

    Json(json!({
        "llm_configured": llm_configured,
        "embed_configured": embed_configured,
        "llm_provider": state.config.llm_provider,
        "embed_provider": state.config.embed_provider,
    }))
}
