use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API error: {status} — {message}")]
    Api { status: u16, message: String },
    #[error("stream error: {0}")]
    Stream(String),
    #[error("provider not configured")]
    NotConfigured,
}

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("API error: {status} — {message}")]
    Api { status: u16, message: String },
    #[error("provider not configured")]
    NotConfigured,
}
