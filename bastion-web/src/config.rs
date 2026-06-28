use std::path::PathBuf;

use crate::error::{WebError, WebResult};

#[derive(Debug, Clone)]
pub struct Config {
    pub wiki_path: PathBuf,
    pub raw_path: PathBuf,
    pub db_path: PathBuf,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub embed_provider: Option<String>,
    pub embed_model: Option<String>,
    pub openai_key: Option<String>,
    pub anthropic_key: Option<String>,
    pub voyage_key: Option<String>,
    pub openrouter_key: Option<String>,
    pub gemini_key: Option<String>,
    pub git_author_name: String,
    pub git_author_email: String,
    pub port: u16,
}

impl Config {
    /// Load from environment variables. Fails if required variables are absent.
    pub fn from_env() -> WebResult<Self> {
        dotenvy::dotenv().ok();

        let wiki_path = PathBuf::from(require_env("BASTION_WIKI_PATH")?);
        let raw_path = PathBuf::from(require_env("BASTION_RAW_PATH")?);
        let db_path = PathBuf::from(require_env("BASTION_DB_PATH")?);

        let port: u16 = std::env::var("BASTION_BACKEND_PORT")
            .unwrap_or_else(|_| "8080".into())
            .parse()
            .map_err(|_| {
                WebError::Config("BASTION_BACKEND_PORT must be a valid port number".into())
            })?;

        let git_author_name =
            std::env::var("BASTION_GIT_AUTHOR_NAME").unwrap_or_else(|_| "Bastion".into());
        let git_author_email =
            std::env::var("BASTION_GIT_AUTHOR_EMAIL").unwrap_or_else(|_| "bastion@local".into());

        let llm_provider = std::env::var("BASTION_LLM_PROVIDER")
            .ok()
            .filter(|s| !s.is_empty());
        let llm_model = std::env::var("BASTION_LLM_MODEL")
            .ok()
            .filter(|s| !s.is_empty());
        let embed_provider = std::env::var("BASTION_EMBED_PROVIDER")
            .ok()
            .filter(|s| !s.is_empty());
        let embed_model = std::env::var("BASTION_EMBED_MODEL")
            .ok()
            .filter(|s| !s.is_empty());
        let openai_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let voyage_key = std::env::var("VOYAGE_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let openrouter_key = std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let gemini_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        Ok(Self {
            wiki_path,
            raw_path,
            db_path,
            llm_provider,
            llm_model,
            embed_provider,
            embed_model,
            openai_key,
            anthropic_key,
            voyage_key,
            openrouter_key,
            gemini_key,
            git_author_name,
            git_author_email,
            port,
        })
    }
}

fn require_env(name: &str) -> WebResult<String> {
    std::env::var(name)
        .map_err(|_| WebError::Config(format!("required environment variable {name} is not set")))
}
