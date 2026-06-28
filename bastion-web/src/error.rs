use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bastion_llm::LlmError;
use bastion_review::ReviewError;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebError {
    #[error("wiki error: {0}")]
    Wiki(#[from] bastion_wiki::WikiError),

    #[error("store error: {0}")]
    Store(#[from] bastion_store::StoreError),

    #[error("LLM provider not configured")]
    LlmNotConfigured,

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("API key inválida, expirada ou sem permissão para {provider}/{model}")]
    LlmAuthError {
        provider: String,
        model: String,
        provider_error: String,
    },

    #[error("cota ou créditos esgotados em {provider}/{model}")]
    LlmQuotaError {
        provider: String,
        model: String,
        provider_error: String,
    },

    #[error("rate limit atingido em {provider}/{model}")]
    LlmRateLimited {
        provider: String,
        model: String,
        provider_error: String,
    },

    #[error("provider de LLM indisponível: {provider}/{model}")]
    LlmProviderUnavailable {
        provider: String,
        model: String,
        provider_error: String,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("configuration error: {0}")]
    Config(String),
}

pub type WebResult<T> = Result<T, WebError>;

impl From<ReviewError> for WebError {
    fn from(e: ReviewError) -> Self {
        match e {
            ReviewError::Llm(e) => Self::LlmError(e.to_string()),
            ReviewError::Wiki(e) => Self::Wiki(e),
            ReviewError::Store(e) => Self::Store(e),
            ReviewError::Core(s) => Self::BadRequest(s),
        }
    }
}

impl WebError {
    pub fn from_llm_error(error: LlmError, provider: &str, model: &str) -> Self {
        match error {
            LlmError::NotConfigured => Self::LlmNotConfigured,
            LlmError::Api { status, message } => {
                classify_llm_api_error(status, &message, provider, model)
            }
            LlmError::Http(error) => Self::LlmProviderUnavailable {
                provider: provider.to_string(),
                model: model.to_string(),
                provider_error: error.to_string(),
            },
            other => Self::LlmError(other.to_string()),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::LlmNotConfigured => "LLM_NOT_CONFIGURED",
            Self::LlmAuthError { .. } => "LLM_AUTH_ERROR",
            Self::LlmQuotaError { .. } => "LLM_QUOTA_ERROR",
            Self::LlmRateLimited { .. } => "LLM_RATE_LIMITED",
            Self::LlmProviderUnavailable { .. } => "LLM_PROVIDER_UNAVAILABLE",
            Self::LlmError(_) => "LLM_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::BadRequest(_) => "BAD_REQUEST",
            Self::Config(_) => "CONFIG_ERROR",
            _ => "INTERNAL_ERROR",
        }
    }
}

fn classify_llm_api_error(status: u16, message: &str, provider: &str, model: &str) -> WebError {
    let lower = message.to_lowercase();
    let provider = provider.to_string();
    let model = model.to_string();
    let provider_error = message.to_string();
    match status {
        401 | 403 => WebError::LlmAuthError {
            provider,
            model,
            provider_error,
        },
        402 => WebError::LlmQuotaError {
            provider,
            model,
            provider_error,
        },
        429 if lower.contains("quota") || lower.contains("credit") || lower.contains("billing") => {
            WebError::LlmQuotaError {
                provider,
                model,
                provider_error,
            }
        }
        429 => WebError::LlmRateLimited {
            provider,
            model,
            provider_error,
        },
        500..=599 => WebError::LlmProviderUnavailable {
            provider,
            model,
            provider_error,
        },
        _ => WebError::LlmError(message.to_string()),
    }
}

impl WebError {
    pub fn llm_details(&self) -> Option<(&str, &str, &str)> {
        match self {
            Self::LlmAuthError {
                provider,
                model,
                provider_error,
            }
            | Self::LlmQuotaError {
                provider,
                model,
                provider_error,
            }
            | Self::LlmRateLimited {
                provider,
                model,
                provider_error,
            }
            | Self::LlmProviderUnavailable {
                provider,
                model,
                provider_error,
            } => Some((provider, model, provider_error)),
            _ => None,
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::LlmNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "LLM_NOT_CONFIGURED",
                self.to_string(),
            ),
            Self::LlmAuthError { .. } => {
                (StatusCode::UNAUTHORIZED, "LLM_AUTH_ERROR", self.to_string())
            }
            Self::LlmQuotaError { .. } => (
                StatusCode::PAYMENT_REQUIRED,
                "LLM_QUOTA_ERROR",
                self.to_string(),
            ),
            Self::LlmRateLimited { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "LLM_RATE_LIMITED",
                self.to_string(),
            ),
            Self::LlmProviderUnavailable { .. } => (
                StatusCode::BAD_GATEWAY,
                "LLM_PROVIDER_UNAVAILABLE",
                self.to_string(),
            ),
            Self::LlmError(_) => (StatusCode::BAD_GATEWAY, "LLM_ERROR", self.to_string()),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND", self.to_string()),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", self.to_string()),
            Self::Config(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIG_ERROR",
                self.to_string(),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                self.to_string(),
            ),
        };

        let mut body = json!({ "error": message, "code": code });
        if let Some((provider, model, provider_error)) = self.llm_details() {
            body["provider"] = json!(provider);
            body["model"] = json!(model);
            body["provider_error"] = json!(provider_error);
        }

        (status, Json(body)).into_response()
    }
}
