use async_trait::async_trait;

use crate::error::EmbedError;

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dimensions(&self) -> usize;
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
}
