use thiserror::Error;

#[derive(Debug, Error)]
pub enum BastionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid wiki path: {0}")]
    InvalidPath(String),

    #[error("page not found: {0}")]
    PageNotFound(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("LLM provider not configured")]
    LlmNotConfigured,

    #[error("configuration error: {0}")]
    Config(String),
}

pub type BastionResult<T> = Result<T, BastionError>;
