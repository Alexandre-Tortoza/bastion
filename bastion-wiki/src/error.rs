use thiserror::Error;

#[derive(Debug, Error)]
pub enum WikiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid wiki path: {0}")]
    InvalidPath(String),

    #[error("page not found: {0}")]
    PageNotFound(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("tempfile error: {0}")]
    TempFile(#[from] tempfile::PersistError),
}

pub type WikiResult<T> = Result<T, WikiError>;

impl From<bastion_core::BastionError> for WikiError {
    fn from(e: bastion_core::BastionError) -> Self {
        match e {
            bastion_core::BastionError::Io(e) => Self::Io(e),
            bastion_core::BastionError::Json(e) => Self::Json(e),
            bastion_core::BastionError::InvalidPath(s) => Self::InvalidPath(s),
            bastion_core::BastionError::PageNotFound(s) => Self::PageNotFound(s),
            bastion_core::BastionError::Git(s) => Self::Git(s),
            other => Self::Git(other.to_string()),
        }
    }
}
