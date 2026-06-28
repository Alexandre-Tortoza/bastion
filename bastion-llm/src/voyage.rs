//! Voyage embedder.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::embedder::Embedder;
use crate::error::EmbedError;

const VOYAGE_EMBED_URL: &str = "https://api.voyageai.com/v1/embeddings";

pub struct VoyageEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dimensions: usize,
}

impl VoyageEmbedder {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>, dimensions: usize) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
            model: model.into(),
            dimensions,
        }
    }
}

#[derive(Deserialize)]
struct VoyageData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct VoyageResponse {
    data: Vec<VoyageData>,
}

#[async_trait]
impl Embedder for VoyageEmbedder {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        let body = json!({ "model": self.model, "input": texts });
        let resp = self
            .client
            .post(VOYAGE_EMBED_URL)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let text = resp.text().await?;

        if status != 200 {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v["detail"].as_str().map(String::from))
                .unwrap_or(text);
            return Err(EmbedError::Api { status, message });
        }

        let parsed: VoyageResponse = serde_json::from_str(&text)?;
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
    fn provider_id(&self) -> &str {
        "voyage"
    }
    fn model_id(&self) -> &str {
        &self.model
    }
}
