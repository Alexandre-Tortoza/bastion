//! Ingest upload + status handlers.

use std::process::Command;

use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{WebError, WebResult};
use crate::state::{AppState, IngestJob, IngestStep};

// ── POST /api/ingest/upload ───────────────────────────────────────────────────

pub async fn ingest_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> WebResult<(StatusCode, Json<serde_json::Value>)> {
    if state.get_llm().is_none() {
        return Err(WebError::LlmNotConfigured);
    }

    // Extract the file field.
    let mut filename = String::new();
    let mut file_bytes = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| WebError::BadRequest(format!("multipart error: {e}")))?
    {
        if field.name() == Some("file") {
            filename = field.file_name().unwrap_or("upload.pdf").to_string();
            file_bytes = field
                .bytes()
                .await
                .map_err(|e| WebError::BadRequest(format!("failed to read file: {e}")))?
                .to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err(WebError::BadRequest("field 'file' is required".into()));
    }

    // Derive slug from filename.
    let stem = std::path::Path::new(&filename)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "upload".into());
    let slug = slug_from(&stem);

    let job_id = Uuid::new_v4().to_string();
    let raw_dir = state.config.raw_path.join("papers").join(&slug);

    // Register job.
    {
        let mut jobs = state.jobs.lock().unwrap();
        jobs.insert(
            job_id.clone(),
            IngestJob {
                id: job_id.clone(),
                step: IngestStep::Received,
                done: false,
                error: None,
                wiki_path: None,
            },
        );
    }

    // Spawn background task.
    let state2 = state.clone();
    let job_id2 = job_id.clone();
    tokio::spawn(async move {
        run_pipeline(state2, job_id2, raw_dir, file_bytes, slug).await;
    });

    Ok((StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))))
}

async fn run_pipeline(
    state: AppState,
    job_id: String,
    raw_dir: std::path::PathBuf,
    pdf_bytes: Vec<u8>,
    slug: String,
) {
    macro_rules! set_step {
        ($step:expr) => {
            if let Ok(mut jobs) = state.jobs.lock() {
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.step = $step;
                }
            }
        };
    }
    macro_rules! fail {
        ($msg:expr) => {{
            warn!(job = %job_id, error = $msg, "ingest pipeline failed");
            if let Ok(mut jobs) = state.jobs.lock() {
                if let Some(job) = jobs.get_mut(&job_id) {
                    job.done = true;
                    job.error = Some($msg.to_string());
                }
            }
            return;
        }};
    }

    // [Converting] — write PDF, run bastion-ingest subprocess.
    set_step!(IngestStep::Converting);

    if let Err(e) = std::fs::create_dir_all(&raw_dir) {
        fail!(format!("cannot create raw dir: {e}"));
    }

    let pdf_path = raw_dir.join("original.pdf");
    if let Err(e) = std::fs::write(&pdf_path, &pdf_bytes) {
        fail!(format!("cannot write PDF: {e}"));
    }

    let ingest_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("bastion-ingest")))
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("bastion-ingest"));

    let output = Command::new(&ingest_bin)
        .arg("--input")
        .arg(&pdf_path)
        .arg("--output-dir")
        .arg(&raw_dir)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            fail!(format!(
                "bastion-ingest failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
        }
        Err(e) => {
            fail!(format!("cannot run bastion-ingest: {e}"));
        }
    };

    let ingest_result: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            fail!(format!("bad bastion-ingest output: {e}"));
        }
    };
    info!(job = %job_id, ?ingest_result, "bastion-ingest done");

    // [Extracting] — LLM reads original.md, produces extracted-notes.md.
    set_step!(IngestStep::Extracting);

    let original_md = match std::fs::read_to_string(raw_dir.join("original.md")) {
        Ok(s) => s,
        Err(e) => {
            fail!(format!("cannot read original.md: {e}"));
        }
    };

    let llm = state.get_llm().unwrap();

    let extraction_prompt = format!(
        "Você é um assistente de pesquisa. Leia o artigo científico abaixo e extraia:\n\
         1. Título, autores, ano, venue\n\
         2. Problema abordado\n\
         3. Metodologia principal\n\
         4. Resultados e métricas\n\
         5. Limitações declaradas\n\
         6. Contribuições principais (bulleted)\n\
         7. Citações relevantes (com número de página)\n\n\
         Responda em Markdown estruturado.\n\n---\n\n{original_md}"
    );

    let notes = match llm
        .chat(
            vec![bastion_llm::Message {
                role: bastion_llm::Role::User,
                content: extraction_prompt,
            }],
            bastion_llm::ChatOptions {
                max_tokens: Some(4096),
                ..Default::default()
            },
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            fail!(format!("LLM extraction failed: {e}"));
        }
    };

    if let Err(e) = std::fs::write(raw_dir.join("extracted-notes.md"), &notes) {
        fail!(format!("cannot write extracted-notes.md: {e}"));
    }

    // [Integrating] — LLM writes wiki pages.
    set_step!(IngestStep::Integrating);

    let index_raw = state
        .wiki
        .read_raw(&bastion_core::WikiPath::new("index.md").unwrap())
        .unwrap_or_default();

    let integration_prompt = format!(
        "Você é um assistente de pesquisa. Com base nas notas extraídas e na wiki existente, \
         crie a página wiki para este artigo em `papers/{slug}.md`.\n\n\
         Siga o formato:\n\
         ```\n\
         ---\n\
         title: <título completo>\n\
         kind: paper\n\
         tier: episodic\n\
         authors: [<autores>]\n\
         year: <ano>\n\
         venue: <venue>\n\
         status: ingested\n\
         ---\n\n\
         ## Resumo\n...\n\n## Metodologia\n...\n\n## Resultados\n...\n\n## Limitações\n...\n\
         ```\n\n\
         Wiki index atual:\n{index_raw}\n\n\
         Notas extraídas:\n{notes}"
    );

    let wiki_content = match llm
        .chat(
            vec![bastion_llm::Message {
                role: bastion_llm::Role::User,
                content: integration_prompt,
            }],
            bastion_llm::ChatOptions {
                max_tokens: Some(4096),
                ..Default::default()
            },
        )
        .await
    {
        Ok(s) => s,
        Err(e) => {
            fail!(format!("LLM integration failed: {e}"));
        }
    };

    let wiki_page_path = format!("papers/{slug}.md");
    let path = match bastion_core::WikiPath::new(&wiki_page_path) {
        Ok(p) => p,
        Err(e) => {
            fail!(format!("invalid wiki path: {e}"));
        }
    };

    // Strip markdown code fence if LLM wrapped the output.
    let content = strip_code_fence(&wiki_content);

    // Parse frontmatter from LLM output to get a proper frontmatter value.
    let frontmatter = serde_json::json!({
        "title": slug,
        "kind": "paper",
        "tier": "episodic",
        "status": "ingested"
    });

    if let Err(e) = state.wiki.write_page(bastion_core::WritePageRequest {
        path: path.clone(),
        frontmatter,
        body: content,
        action: bastion_core::CommitAction::Ingest,
        scope: "papers".into(),
        subject: format!("add {slug}"),
    }) {
        fail!(format!("wiki write failed: {e}"));
    }

    // [Indexed] — sync store.
    set_step!(IngestStep::Indexed);

    if let Err(e) = state.store.sync_from_wiki(&state.config.wiki_path) {
        warn!(job = %job_id, error = %e, "sync_from_wiki failed after ingest");
    }

    // [Embedding] — embed new/stale pages if embedder configured.
    if state.get_embedder().is_some() {
        set_step!(IngestStep::Embedding);
        embed_pages(&state).await;
    }

    // Mark done.
    if let Ok(mut jobs) = state.jobs.lock() {
        if let Some(job) = jobs.get_mut(&job_id) {
            job.done = true;
            job.wiki_path = Some(wiki_page_path);
        }
    }
    info!(job = %job_id, slug, "ingest complete");
}

fn strip_code_fence(s: &str) -> String {
    let s = s.trim();
    if let Some(rest) = s
        .strip_prefix("```markdown\n")
        .or_else(|| s.strip_prefix("```\n"))
    {
        if let Some(inner) = rest
            .strip_suffix("\n```")
            .or_else(|| rest.strip_suffix("```"))
        {
            return inner.to_string();
        }
    }
    s.to_string()
}

fn slug_from(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Embedding helper (shared by pipeline + backfill) ─────────────────────────

pub(crate) async fn embed_pages(state: &AppState) {
    let Some(embedder) = state.get_embedder() else {
        return;
    };
    let provider = embedder.provider_id().to_string();
    let model = embedder.model_id().to_string();
    let dim = embedder.dimensions();

    let to_embed = match state.store.pages_needing_embed(&provider, &model) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "pages_needing_embed failed");
            return;
        }
    };

    if to_embed.is_empty() {
        return;
    }
    info!(count = to_embed.len(), "embedding pages");

    for chunk in to_embed.chunks(20) {
        let texts: Vec<String> = chunk.iter().map(|(_, _, body)| body.clone()).collect();
        match embedder.embed(texts).await {
            Ok(vecs) => {
                for ((_, path, body), vec) in chunk.iter().zip(vecs.iter()) {
                    if let Err(e) = state
                        .store
                        .upsert_embedding(path, &provider, &model, dim, vec, body)
                    {
                        warn!(path, error = %e, "upsert_embedding failed");
                    }
                }
            }
            Err(e) => warn!(error = %e, "embed batch failed"),
        }
    }
}

// ── POST /api/embeddings/backfill ─────────────────────────────────────────────

pub async fn backfill_embeddings(
    State(state): State<AppState>,
) -> WebResult<Json<serde_json::Value>> {
    if state.get_embedder().is_none() {
        return Err(WebError::BadRequest("no embedder configured".into()));
    }

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().unwrap();
        jobs.insert(
            job_id.clone(),
            IngestJob {
                id: job_id.clone(),
                step: IngestStep::Embedding,
                done: false,
                error: None,
                wiki_path: None,
            },
        );
    }

    let state2 = state.clone();
    let job_id2 = job_id.clone();
    tokio::spawn(async move {
        embed_pages(&state2).await;
        if let Ok(mut jobs) = state2.jobs.lock() {
            if let Some(job) = jobs.get_mut(&job_id2) {
                job.done = true;
            }
        }
    });

    Ok(Json(json!({ "job_id": job_id })))
}

// ── GET /api/ingest/status/:job_id ───────────────────────────────────────────

pub async fn ingest_status(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> WebResult<Json<serde_json::Value>> {
    let jobs = state.jobs.lock().unwrap();
    let job = jobs
        .get(&job_id)
        .ok_or_else(|| WebError::NotFound(format!("job not found: {job_id}")))?;

    Ok(Json(serde_json::to_value(job).unwrap()))
}
