//! Anthropic provider (chat with streaming).

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::error::LlmError;
use crate::provider::{ChatOptions, LlmProvider, Message, Role, TokenStream};

const ANTHROPIC_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    fn build_body(&self, messages: &[Message], opts: &ChatOptions, stream: bool) -> Value {
        // Anthropic separates system prompt from the messages array.
        let system: Option<&str> = messages
            .iter()
            .find(|m| matches!(m.role, Role::System))
            .map(|m| m.content.as_str());

        let msgs: Vec<Value> = messages
            .iter()
            .filter(|m| !matches!(m.role, Role::System))
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::System => unreachable!(),
                };
                json!({ "role": role, "content": m.content })
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
            "max_tokens": opts.max_tokens.unwrap_or(4096),
        });
        if let Some(s) = system {
            body["system"] = json!(s);
        }
        if let Some(t) = opts.temperature {
            body["temperature"] = json!(t);
        }
        if stream {
            body["stream"] = json!(true);
        }
        body
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        "anthropic"
    }

    fn model_id(&self) -> &str {
        &self.model
    }

    async fn chat(&self, messages: Vec<Message>, opts: ChatOptions) -> Result<String, LlmError> {
        let body = self.build_body(&messages, &opts, false);
        let resp = self
            .client
            .post(ANTHROPIC_MESSAGES_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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
        let content = parsed["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        debug!(model = %self.model, len = content.len(), "anthropic chat complete");
        Ok(content)
    }

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        opts: ChatOptions,
    ) -> Result<TokenStream, LlmError> {
        let body = self.build_body(&messages, &opts, true);
        let resp = self
            .client
            .post(ANTHROPIC_MESSAGES_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
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

        // Anthropic SSE: event: content_block_delta / data: {"delta":{"text":"..."}}
        let byte_stream = resp.bytes_stream();
        let token_stream = byte_stream.filter_map(|chunk: Result<Bytes, _>| async move {
            let chunk = chunk.ok()?;
            let text = std::str::from_utf8(&chunk).ok()?.to_string();
            let mut tokens = Vec::new();
            for line in text.lines() {
                let line = line.strip_prefix("data: ").unwrap_or(line);
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line) {
                    Ok(v) if v["type"] == "content_block_delta" => {
                        if let Some(t) = v["delta"]["text"].as_str() {
                            tokens.push(t.to_string());
                        }
                    }
                    Ok(_) => {}
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
