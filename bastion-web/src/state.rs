use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use bastion_llm::{Embedder, LlmProvider};
use bastion_store::Store;
use bastion_wiki::Wiki;
use serde::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IngestStep {
    Received,
    Converting,
    Extracting,
    Integrating,
    Indexed,
    Embedding,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestJob {
    pub id: String,
    pub step: IngestStep,
    pub done: bool,
    pub error: Option<String>,
    pub wiki_path: Option<String>,
}

pub struct ProviderOverrides {
    pub llm: Option<Arc<dyn LlmProvider>>,
    pub embedder: Option<Arc<dyn Embedder>>,
}

#[derive(Clone)]
pub struct AppState {
    pub wiki: Arc<Wiki>,
    pub store: Arc<Store>,
    pub config: Arc<Config>,
    pub llm: Option<Arc<dyn LlmProvider>>,
    pub embedder: Option<Arc<dyn Embedder>>,
    pub overrides: Arc<RwLock<ProviderOverrides>>,
    pub jobs: Arc<Mutex<HashMap<String, IngestJob>>>,
}

impl AppState {
    pub fn get_llm(&self) -> Option<Arc<dyn LlmProvider>> {
        self.overrides
            .read()
            .unwrap()
            .llm
            .clone()
            .or_else(|| self.llm.clone())
    }

    pub fn get_embedder(&self) -> Option<Arc<dyn Embedder>> {
        self.overrides
            .read()
            .unwrap()
            .embedder
            .clone()
            .or_else(|| self.embedder.clone())
    }
}
