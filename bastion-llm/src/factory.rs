//! Build LLM providers from config/env strings.

use std::sync::Arc;

use crate::anthropic::AnthropicProvider;
use crate::embedder::Embedder;
use crate::openai::{OpenAiEmbedder, OpenAiProvider};
use crate::provider::LlmProvider;
use crate::voyage::VoyageEmbedder;

/// Build a chat LLM provider from provider name + model string.
///
/// Returns `None` if `provider` is `None` or empty.
///
/// # Panics
/// Panics if the provider requires an API key that is absent.
pub fn build_llm_provider(
    provider: Option<&str>,
    model: Option<&str>,
    openai_key: Option<&str>,
    anthropic_key: Option<&str>,
    openrouter_key: Option<&str>,
    gemini_key: Option<&str>,
) -> Option<Arc<dyn LlmProvider>> {
    let provider = provider.filter(|s| !s.is_empty())?;

    match provider {
        "openai" => {
            let key = openai_key
                .filter(|s| !s.is_empty())
                .expect("OPENAI_API_KEY required when BASTION_LLM_PROVIDER=openai");
            let model = model.unwrap_or("gpt-4o");
            Some(Arc::new(OpenAiProvider::new(key, model)))
        }
        "anthropic" => {
            let key = anthropic_key
                .filter(|s| !s.is_empty())
                .expect("ANTHROPIC_API_KEY required when BASTION_LLM_PROVIDER=anthropic");
            let model = model.unwrap_or("claude-sonnet-4-6");
            Some(Arc::new(AnthropicProvider::new(key, model)))
        }
        "openrouter" => {
            let key = openrouter_key
                .filter(|s| !s.is_empty())
                .expect("OPENROUTER_API_KEY required when BASTION_LLM_PROVIDER=openrouter");
            let model = model.unwrap_or("openai/gpt-4o");
            Some(Arc::new(OpenAiProvider::with_provider_id(
                key,
                model,
                "https://openrouter.ai/api/v1",
                "openrouter",
            )))
        }
        "gemini" => {
            let key = gemini_key
                .filter(|s| !s.is_empty())
                .expect("GEMINI_API_KEY required when BASTION_LLM_PROVIDER=gemini");
            let model = model.unwrap_or("gemini-2.0-flash");
            Some(Arc::new(OpenAiProvider::with_provider_id(
                key,
                model,
                "https://generativelanguage.googleapis.com/v1beta/openai",
                "gemini",
            )))
        }
        other => panic!(
            "unknown LLM provider: {other}. Expected 'openai', 'anthropic', 'openrouter' or 'gemini'"
        ),
    }
}

/// Build an embedder from provider name + model string.
///
/// Returns `None` if `provider` is `None` or empty.
pub fn build_embedder(
    provider: Option<&str>,
    model: Option<&str>,
    openai_key: Option<&str>,
    voyage_key: Option<&str>,
    gemini_key: Option<&str>,
) -> Option<Arc<dyn Embedder>> {
    let provider = provider.filter(|s| !s.is_empty())?;

    match provider {
        "openai" => {
            let key = openai_key
                .filter(|s| !s.is_empty())
                .expect("OPENAI_API_KEY required when BASTION_EMBED_PROVIDER=openai");
            let model = model.unwrap_or("text-embedding-3-small");
            let dims = if model.contains("3-large") {
                3072
            } else {
                1536
            };
            Some(Arc::new(OpenAiEmbedder::new(key, model, dims)))
        }
        "voyage" => {
            let key = voyage_key
                .filter(|s| !s.is_empty())
                .expect("VOYAGE_API_KEY required when BASTION_EMBED_PROVIDER=voyage");
            let model = model.unwrap_or("voyage-3");
            let dims = if model.contains("lite") { 512 } else { 1024 };
            Some(Arc::new(VoyageEmbedder::new(key, model, dims)))
        }
        "gemini" => {
            let key = gemini_key
                .filter(|s| !s.is_empty())
                .expect("GEMINI_API_KEY required when BASTION_EMBED_PROVIDER=gemini");
            let model = model.unwrap_or("text-embedding-004");
            Some(Arc::new(OpenAiEmbedder::with_base_url(
                key,
                model,
                768,
                "https://generativelanguage.googleapis.com/v1beta/openai",
            )))
        }
        other => panic!("unknown embed provider: {other}. Expected 'openai', 'voyage' or 'gemini'"),
    }
}
