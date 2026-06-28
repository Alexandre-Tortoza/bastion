//! OpenAI provider (chat + embeddings).

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::embedder::Embedder;
use crate::error::{EmbedError, LlmError};
use crate::provider::{ChatOptions, LlmProvider, Message, Role, TokenStream};

// ── Chat Provider ─────────────────────────────────────────────────────────────

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    provider_id: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::with_base_url(api_key, model, "https://api.openai.com/v1")
    }

    pub fn with_base_url(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let base_url = base_url.into();
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            provider_id: provider_id_from_base_url(&base_url).to_string(),
            base_url,
        }
    }

    pub fn with_provider_id(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
            provider_id: provider_id.into(),
        }
    }
}

fn provider_id_from_base_url(base_url: &str) -> &'static str {
    if base_url.contains("openrouter") {
        "openrouter"
    } else if base_url.contains("generativelanguage") {
        "gemini"
    } else {
        "openai"
    }
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

fn role_str(r: &Role) -> &'static str {
    match r {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn provider_id(&self) -> &str {
        &self.provider_id
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    async fn chat(&self, messages: Vec<Message>, opts: ChatOptions) -> Result<String, LlmError> {
        let msgs: Vec<OpenAiMessage<'_>> = messages
            .iter()
            .map(|m| OpenAiMessage {
                role: role_str(&m.role),
                content: &m.content,
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
        });
        if let Some(t) = opts.max_tokens {
            body["max_tokens"] = json!(t);
        }
        if let Some(t) = opts.temperature {
            body["temperature"] = json!(t);
        }

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;

        if status != 200 {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            return Err(LlmError::Api { status, message });
        }

        let parsed: Value = serde_json::from_str(&text)?;
        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        debug!(model = %self.model, len = content.len(), "openai chat complete");
        Ok(content)
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        opts: ChatOptions,
    ) -> Result<TokenStream, LlmError> {
        let msgs: Vec<OpenAiMessage<'_>> = messages
            .iter()
            .map(|m| OpenAiMessage {
                role: role_str(&m.role),
                content: &m.content,
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
            "stream": true,
        });
        if let Some(t) = opts.max_tokens {
            body["max_tokens"] = json!(t);
        }
        if let Some(t) = opts.temperature {
            body["temperature"] = json!(t);
        }

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status != 200 {
            let text = resp.text().await?;
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            return Err(LlmError::Api { status, message });
        }

        // Parse SSE stream: each line "data: {...}" or "data: [DONE]"
        let byte_stream = resp.bytes_stream();
        let token_stream = byte_stream.filter_map(|chunk: Result<Bytes, _>| async move {
            let chunk = chunk.ok()?;
            let text = std::str::from_utf8(&chunk).ok()?.to_string();
            let mut tokens = Vec::new();
            for line in text.lines() {
                let line = line.strip_prefix("data: ").unwrap_or(line);
                if line == "[DONE]" || line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line) {
                    Ok(v) => {
                        if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                            tokens.push(t.to_string());
                        }
                    }
                    Err(e) => warn!(error = %e, "failed to parse SSE line"),
                }
            }
            if tokens.is_empty() {
                None
            } else {
                Some(Ok(tokens.join("")))
            }
        });

        Ok(Box::pin(token_stream) as TokenStream)
    }
}

// ── Embedder ──────────────────────────────────────────────────────────────────

pub struct OpenAiEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
    base_url: String,
}

impl OpenAiEmbedder {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, dimensions: usize) -> Self {
        Self::with_base_url(api_key, model, dimensions, "https://api.openai.com/v1")
    }

    pub fn with_base_url(
        api_key: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            dimensions,
            base_url: base_url.into(),
        }
    }
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[async_trait]
impl Embedder for OpenAiEmbedder {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        let body = json!({ "model": self.model, "input": texts });
        let resp = self
            .client
            .post(format!("{}/embeddings", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;

        if status != 200 {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["error"]["message"].as_str().map(String::from))
                .unwrap_or(text);
            return Err(EmbedError::Api { status, message });
        }

        let parsed: EmbedResponse = serde_json::from_str(&text)?;
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
    fn provider_id(&self) -> &str {
        "openai"
    }
    fn model_id(&self) -> &str {
        &self.model
    }
}
