use bastion_llm::LlmError;
use bastion_store::StoreError;
use bastion_wiki::WikiError;

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),
    #[error("wiki error: {0}")]
    Wiki(#[from] WikiError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    Core(String),
}

pub type ReviewResult<T> = Result<T, ReviewError>;
