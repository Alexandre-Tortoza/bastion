//! Review (LaTeX analysis) and lint endpoints.

use axum::{Json, extract::State};
use bastion_core::{CommitAction, PageFilter, PageKind, WikiPath, WritePageRequest};
use bastion_review::{LintRunner, ReviewEngine};
use serde::Deserialize;
use serde_json::json;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

// ── POST /api/review/analyze ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AnalyzeBody {
    pub latex: String,
    pub slug: Option<String>,
}

struct ReviewContextHit {
    path: String,
    title: String,
    kind: Option<String>,
    snippet: String,
    body: String,
}

async fn retrieve_review_context(state: &AppState, latex: &str) -> Vec<ReviewContextHit> {
    let query = latex_query(latex);
    let query_vec = if let Some(embedder) = state.get_embedder() {
        embedder
            .embed(vec![query.clone()])
            .await
            .ok()
            .and_then(|mut v| v.pop())
    } else {
        None
    };

    let hits = if let Some(embedder) = state.get_embedder() {
        state
            .store
            .hybrid_search(
                &query,
                query_vec.as_deref(),
                embedder.provider_id(),
                embedder.model_id(),
                embedder.dimensions(),
                8,
            )
            .unwrap_or_default()
    } else {
        state.store.fts_search(&query, 8).unwrap_or_default()
    };

    hits.into_iter()
        .filter_map(|hit| {
            let path = WikiPath::new(hit.path.clone()).ok()?;
            let page = state.wiki.read_page(&path).ok()?;
            Some(ReviewContextHit {
                path: hit.path,
                title: hit.title,
                kind: hit.kind,
                snippet: hit.snippet,
                body: page.body,
            })
        })
        .collect()
}

fn latex_query(latex: &str) -> String {
    let cleaned = latex
        .lines()
        .filter(|line| !line.trim_start().starts_with('%'))
        .map(|line| {
            line.replace('\\', " ")
                .replace('{', " ")
                .replace('}', " ")
                .replace('$', " ")
        })
        .collect::<Vec<_>>()
        .join(" ");

    cleaned
        .split_whitespace()
        .take(180)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_review_context(hits: &[ReviewContextHit]) -> String {
    hits.iter()
        .map(|hit| {
            let body = hit.body.chars().take(3_000).collect::<String>();
            format!(
                "### {} ({})\nKind: {}\nSnippet: {}\n\n{}",
                hit.title,
                hit.path,
                hit.kind.as_deref().unwrap_or("unknown"),
                hit.snippet,
                body
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

pub async fn analyze_review(
    State(state): State<AppState>,
    Json(body): Json<AnalyzeBody>,
) -> WebResult<Json<serde_json::Value>> {
    let llm = state.get_llm().ok_or(WebError::LlmNotConfigured)?;

    if body.latex.trim().is_empty() {
        return Err(WebError::BadRequest("latex is required".into()));
    }

    let engine = ReviewEngine {
        llm,
        wiki: state.wiki.clone(),
    };

    let context_hits = retrieve_review_context(&state, &body.latex).await;
    let wiki_context = render_review_context(&context_hits);

    let out = engine
        .analyze_latex(&body.latex, body.slug.as_deref(), Some(&wiki_context))
        .await
        .map_err(WebError::from)?;

    let references = context_hits
        .iter()
        .map(|hit| {
            json!({
                "path": hit.path,
                "title": hit.title,
                "kind": hit.kind,
                "snippet": hit.snippet
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "wiki_path": out.wiki_path,
        "suggestions_total": out.suggestions_total,
        "suggestions": out.suggestions_raw,
        "references": references
    })))
}

// ── PUT /api/wiki/target ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetTargetBody {
    pub latex: String,
    pub title: Option<String>,
}

fn extract_cite_keys(latex: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = latex;
    while let Some(start) = rest.find(r"\cite{") {
        rest = &rest[start + r"\cite{".len()..];
        if let Some(end) = rest.find('}') {
            for key in rest[..end].split(',') {
                let k = key.trim().to_string();
                if !k.is_empty() {
                    keys.push(k);
                }
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    keys.dedup();
    keys
}

fn fuzzy_match_cite(norm_key: &str, norm_stem: &str) -> bool {
    norm_key.len() >= 3
        && norm_stem.len() >= 3
        && (norm_stem.contains(norm_key) || norm_key.contains(norm_stem))
}

pub async fn set_target_paper(
    State(state): State<AppState>,
    Json(body): Json<SetTargetBody>,
) -> WebResult<Json<serde_json::Value>> {
    use chrono::Local;

    if body.latex.trim().is_empty() {
        return Err(WebError::BadRequest("latex is required".into()));
    }

    let today = Local::now().format("%Y-%m-%d").to_string();
    let title = body.title.as_deref().unwrap_or("Meu Paper (Draft)");

    // Extract \cite{key} patterns and match against wiki papers.
    let cite_keys = extract_cite_keys(&body.latex);
    let paper_pages = state
        .store
        .list_pages(&PageFilter {
            kind: Some(PageKind::Paper),
            ..Default::default()
        })
        .unwrap_or_default();

    let mut matched: Vec<(String, String)> = Vec::new(); // (path_without_md, key)
    let mut unmatched: Vec<String> = Vec::new();
    for key in &cite_keys {
        let norm_key: String = key
            .chars()
            .filter(|c| c.is_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let found = paper_pages.iter().find(|m| {
            let stem = m.path.stem();
            if stem == "my-paper" {
                return false;
            }
            let norm_stem: String = stem
                .chars()
                .filter(|c| c.is_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect();
            fuzzy_match_cite(&norm_key, &norm_stem)
        });
        if let Some(meta) = found {
            let path_no_md = meta.path.as_str().trim_end_matches(".md").to_string();
            matched.push((path_no_md, key.clone()));
        } else {
            unmatched.push(key.clone());
        }
    }

    // Build wiki body: LaTeX in code fence + references section outside fence.
    let mut md_body = format!("## LaTeX Source\n\n```latex\n{}\n```\n", body.latex);

    if !matched.is_empty() || !unmatched.is_empty() {
        md_body.push_str("\n## Referências Detectadas\n\n");
        if !matched.is_empty() {
            md_body.push_str("Papers citados encontrados na wiki:\n\n");
            for (path, key) in &matched {
                md_body.push_str(&format!("- [[{}|{}]]\n", path, key));
            }
        }
        if !unmatched.is_empty() {
            md_body.push_str("\nCitações não encontradas na wiki:\n\n");
            for key in &unmatched {
                md_body.push_str(&format!("- `{}`\n", key));
            }
        }
    }

    let path = WikiPath::new("papers/my-paper.md")
        .map_err(|e| WebError::BadRequest(e.to_string()))?;

    let frontmatter = json!({
        "title": title,
        "kind": "paper",
        "tier": "working",
        "is_target": true,
        "updated_at": today,
    });

    state
        .wiki
        .write_page(WritePageRequest {
            path,
            frontmatter,
            body: md_body,
            action: CommitAction::Update,
            scope: "papers".into(),
            subject: "update my-paper draft".into(),
        })
        .map_err(WebError::Wiki)?;

    state.store.sync_from_wiki(&state.config.wiki_path).ok();

    Ok(Json(json!({
        "path": "papers/my-paper.md",
        "updated_at": today,
        "matched_citations": matched.len(),
        "unmatched_citations": unmatched.len(),
    })))
}

// ── POST /api/lint/run ────────────────────────────────────────────────────────

pub async fn run_lint(State(state): State<AppState>) -> WebResult<Json<serde_json::Value>> {
    let runner = LintRunner {
        store: state.store.clone(),
        wiki: state.wiki.clone(),
    };

    let (issues, report_path) = tokio::task::spawn_blocking(move || runner.run())
        .await
        .map_err(|e| WebError::LlmError(format!("task join: {e}")))?
        .map_err(WebError::from)?;

    let issues_json: Vec<serde_json::Value> = issues
        .iter()
        .map(|i| json!({ "page": i.page, "kind": i.kind, "detail": i.detail }))
        .collect();

    Ok(Json(json!({
        "report_path": report_path,
        "issues_total": issues.len(),
        "issues": issues_json,
    })))
}
