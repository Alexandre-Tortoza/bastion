use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    Router,
    routing::{get, post, put},
};
use bastion_llm::{build_embedder, build_llm_provider};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

mod config;
mod error;
mod handlers;
mod state;

use config::Config;
use handlers::{
    config::{get_config, set_config},
    health,
    ingest::{backfill_embeddings, embed_pages, ingest_status, ingest_upload},
    review::{analyze_review, run_lint, set_target_paper},
    wiki::{
        chat_query, create_proposal, delete_page, expand_page, get_log, get_page, graph_data,
        list_decisions, list_pages, list_pending, search_pages,
    },
};
use state::{AppState, ProviderOverrides};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let config = Config::from_env().map_err(|e| anyhow::anyhow!("{e}"))?;
    let port = config.port;

    let store =
        bastion_store::Store::open(&config.db_path).map_err(|e| anyhow::anyhow!("store: {e}"))?;

    let wiki = bastion_wiki::Wiki::new(
        &config.wiki_path,
        &config.git_author_name,
        &config.git_author_email,
    )
    .map_err(|e| anyhow::anyhow!("wiki: {e}"))?;

    let reindexed = store
        .sync_from_wiki(&config.wiki_path)
        .map_err(|e| anyhow::anyhow!("sync: {e}"))?;
    info!(reindexed, "startup sync complete");

    let llm = build_llm_provider(
        config.llm_provider.as_deref(),
        config.llm_model.as_deref(),
        config.openai_key.as_deref(),
        config.anthropic_key.as_deref(),
        config.openrouter_key.as_deref(),
        config.gemini_key.as_deref(),
    );
    let embedder = build_embedder(
        config.embed_provider.as_deref(),
        config.embed_model.as_deref(),
        config.openai_key.as_deref(),
        config.voyage_key.as_deref(),
        config.gemini_key.as_deref(),
    );

    if llm.is_some() {
        info!(
            provider = config.llm_provider.as_deref().unwrap_or(""),
            "LLM provider ready"
        );
    } else {
        info!("no LLM provider configured — chat/ingest/review will return 503");
    }

    let state = AppState {
        wiki: Arc::new(wiki),
        store: Arc::new(store),
        config: Arc::new(config),
        llm,
        embedder,
        overrides: Arc::new(std::sync::RwLock::new(ProviderOverrides {
            llm: None,
            embedder: None,
        })),
        jobs: Arc::new(Mutex::new(HashMap::new())),
    };

    // Backfill embeddings for any pages that are new or stale.
    if let Some(embedder) = state.get_embedder() {
        let count = state
            .store
            .pages_needing_embed(embedder.provider_id(), embedder.model_id())
            .map(|v| v.len())
            .unwrap_or(0);
        if count > 0 {
            info!(count, "spawning startup embedding backfill");
            let state2 = state.clone();
            tokio::spawn(async move { embed_pages(&state2).await });
        } else {
            info!("embeddings up to date");
        }
    }

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/config", get(get_config).post(set_config))
        .route("/api/wiki/graph", get(graph_data))
        .route("/api/wiki/pages", get(list_pages))
        .route("/api/wiki/pages/{*path}", get(get_page).delete(delete_page))
        .route("/api/wiki/decisions", get(list_decisions))
        .route("/api/wiki/log", get(get_log))
        .route("/api/wiki/pending", get(list_pending).post(create_proposal))
        .route("/api/chat/query", post(chat_query))
        .route("/api/wiki/search", get(search_pages))
        .route("/api/wiki/expand", post(expand_page))
        .route("/api/ingest/upload", post(ingest_upload))
        .route("/api/ingest/status/{job_id}", get(ingest_status))
        .route("/api/embeddings/backfill", post(backfill_embeddings))
        .route("/api/review/analyze", post(analyze_review))
        .route("/api/wiki/target", put(set_target_paper))
        .route("/api/lint/run", post(run_lint))
        .nest_service("/raw", ServeDir::new(&state.config.raw_path))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!(%addr, "bastion-web listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
