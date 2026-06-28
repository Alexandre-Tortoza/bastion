use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::error::LlmError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub type TokenStream = Pin<Box<dyn Stream<Item = Result<String, LlmError>> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn provider_id(&self) -> &str;

    fn model_id(&self) -> &str;

    async fn chat(&self, messages: Vec<Message>, opts: ChatOptions) -> Result<String, LlmError>;

    async fn stream_chat(
        &self,
        messages: Vec<Message>,
        opts: ChatOptions,
    ) -> Result<TokenStream, LlmError>;
}
