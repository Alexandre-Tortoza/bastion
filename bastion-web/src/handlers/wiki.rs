//! Wiki read/write handlers + chat SSE.

use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
};
use bastion_core::{CommitAction, PageFilter, PageKind, WikiPath, WritePageRequest};
use bastion_llm::{ChatOptions, Message, Role};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

// ── GET /api/wiki/pages ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListPagesQuery {
    pub kind: Option<String>,
    pub recent: Option<usize>,
}

pub async fn list_pages(
    State(state): State<AppState>,
    Query(q): Query<ListPagesQuery>,
) -> WebResult<Json<serde_json::Value>> {
    let kind = q.kind.as_deref().and_then(|s| PageKind::try_from(s).ok());

    let filter = PageFilter {
        kind,
        limit: q.recent,
        ..Default::default()
    };

    let pages = state.store.list_pages(&filter)?;
    Ok(Json(json!({ "pages": pages })))
}

// ── GET /api/wiki/graph ───────────────────────────────────────────────────────

pub async fn graph_data(State(state): State<AppState>) -> WebResult<Json<serde_json::Value>> {
    let pages = state.store.list_pages(&PageFilter::default())?;
    let links = state.store.get_all_links()?;

    Ok(Json(json!({ "pages": pages, "links": links })))
}

// ── GET /api/wiki/pages/*path ─────────────────────────────────────────────────

pub async fn get_page(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> WebResult<Json<serde_json::Value>> {
    // Normalise: add .md if missing, lowercase.
    let normalised = if path.ends_with(".md") {
        path.clone()
    } else {
        format!("{}.md", path)
    };
    let wiki_path = WikiPath::new(normalised).map_err(|e| WebError::BadRequest(e.to_string()))?;

    let page = state.wiki.read_page(&wiki_path).map_err(|e| match e {
        bastion_wiki::WikiError::PageNotFound(_) => {
            WebError::NotFound(format!("page not found: {path}"))
        }
        other => WebError::Wiki(other),
    })?;

    let backlinks = state
        .store
        .backlinks(wiki_path.as_str())
        .unwrap_or_default();

    let fm = &page.frontmatter;
    Ok(Json(json!({
        "path": page.path.as_str(),
        "title": fm.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        "kind": fm.get("kind").and_then(|v| v.as_str()),
        "tier": fm.get("tier").and_then(|v| v.as_str()),
        "status": fm.get("status").and_then(|v| v.as_str()),
        "pinned": fm.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false),
        "updated_at": fm.get("updated_at").and_then(|v| v.as_str()),
        "created_at": fm.get("created_at").and_then(|v| v.as_str()),
        "tags": fm.get("tags").cloned().unwrap_or(json!([])),
        "frontmatter": fm,
        "body": page.body,
        "wikilinks": page.links,
        "backlinks": backlinks,
    })))
}

// ── DELETE /api/wiki/pages/*path ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DeletePageQuery {
    pub delete_raw: Option<bool>,
}

pub async fn delete_page(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(q): Query<DeletePageQuery>,
) -> WebResult<Json<serde_json::Value>> {
    let normalised = if path.ends_with(".md") {
        path.clone()
    } else {
        format!("{}.md", path)
    };
    let wiki_path = WikiPath::new(&normalised).map_err(|e| WebError::BadRequest(e.to_string()))?;

    state.wiki.delete_page(&wiki_path).map_err(|e| match e {
        bastion_wiki::WikiError::PageNotFound(_) => {
            WebError::NotFound(format!("page not found: {path}"))
        }
        other => WebError::Wiki(other),
    })?;

    state.store.delete_page(&normalised).ok();

    if q.delete_raw.unwrap_or(false) {
        if let Some(dir) = wiki_path.dir() {
            let raw_dir = state.config.raw_path.join(dir).join(wiki_path.stem());
            if raw_dir.exists() {
                std::fs::remove_dir_all(&raw_dir).ok();
            }
        }
    }

    Ok(Json(json!({ "deleted": true, "path": normalised })))
}

// ── POST /api/wiki/expand ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExpandPageBody {
    pub path: String,
}

pub async fn expand_page(
    State(state): State<AppState>,
    Json(body): Json<ExpandPageBody>,
) -> WebResult<Json<serde_json::Value>> {
    let llm = state.get_llm().ok_or(WebError::LlmNotConfigured)?;

    let normalised = if body.path.ends_with(".md") {
        body.path.clone()
    } else {
        format!("{}.md", body.path)
    };
    let wiki_path =
        WikiPath::new(&normalised).map_err(|e| WebError::BadRequest(e.to_string()))?;

    let page = state.wiki.read_page(&wiki_path).map_err(|e| match e {
        bastion_wiki::WikiError::PageNotFound(_) => {
            WebError::NotFound(format!("page not found: {}", body.path))
        }
        other => WebError::Wiki(other),
    })?;

    let slug = wiki_path.stem().to_string();
    let notes = std::fs::read_to_string(
        state
            .config
            .raw_path
            .join("papers")
            .join(&slug)
            .join("extracted-notes.md"),
    )
    .unwrap_or_default();

    let prompt = format!(
        "Você é um assistente de pesquisa. Analise este artigo e crie páginas wiki \
         atômicas cobrindo os conceitos, métodos, resultados e estratégias mais importantes.\n\n\
         Retorne APENAS um JSON array, sem texto extra nem code fences:\n\
         [\n\
           {{\n\
             \"path\": \"concepts/nome-em-kebab-case.md\",\n\
             \"title\": \"Nome do Conceito\",\n\
             \"kind\": \"concept\",\n\
             \"body\": \"## Definição\\n...\\n\\n## Uso no Artigo\\n...\"\n\
           }}\n\
         ]\n\n\
         Regras:\n\
         - kind: \"concept\", \"method\", \"result\" ou \"strategy\"\n\
         - path: lowercase kebab-case no diretório correto:\n\
           concept → concepts/, method → methods/, result → results/, strategy → strategies/\n\
         - body: markdown sem frontmatter, 3-6 frases por seção\n\
         - 6-12 páginas máximo, cobrindo os 4 tipos quando existirem no artigo\n\
         - concept: definição teórica reutilizável (o quê é)\n\
         - method: técnica ou algoritmo com passos replicáveis (como fazer)\n\
         - result: achado empírico específico com métricas e benchmark (o que foi medido)\n\
         - strategy: decisão de design ou abordagem adotada no paper (por que escolheram assim)\n\n\
         Artigo:\n{body}\n\nNotas extraídas:\n{notes}",
        body = page.body,
        notes = notes,
    );

    let response = llm
        .chat(
            vec![Message {
                role: Role::User,
                content: prompt,
            }],
            ChatOptions {
                max_tokens: Some(4096),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| WebError::LlmError(e.to_string()))?;

    let json_str = extract_json_array(&response);
    let pages: Vec<serde_json::Value> = serde_json::from_str(&json_str)
        .map_err(|e| WebError::BadRequest(format!("LLM retornou JSON inválido: {e}")))?;

    let mut created = Vec::new();
    for page_def in &pages {
        let Some(path_str) = page_def["path"].as_str() else {
            continue;
        };
        let Some(title) = page_def["title"].as_str() else {
            continue;
        };
        let kind = page_def["kind"].as_str().unwrap_or("concept");
        let body_str = page_def["body"].as_str().unwrap_or("").to_string();

        let Ok(wp) = WikiPath::new(path_str) else {
            continue;
        };
        let dir = wp.dir().unwrap_or(kind).to_string();
        let source_path = format!("papers/{slug}");
        let frontmatter = serde_json::json!({
            "title": title,
            "kind": kind,
            "tier": "semantic",
            "tags": [],
            "source_paper": source_path,
        });
        let body_with_source = format!("{body_str}\n\n## Fonte\n[[papers/{slug}]]");
        if state
            .wiki
            .write_page(WritePageRequest {
                path: wp,
                frontmatter,
                body: body_with_source,
                action: CommitAction::Update,
                scope: dir,
                subject: format!("atomize from {slug}"),
            })
            .is_ok()
        {
            created.push(path_str.to_string());
        }
    }

    state.store.sync_from_wiki(&state.config.wiki_path).ok();

    Ok(Json(json!({ "created": created })))
}

fn extract_json_array(s: &str) -> String {
    let s = s.trim();
    let inner = if let Some(rest) = s.strip_prefix("```") {
        let rest = rest.trim_start_matches("json").trim_start_matches('\n');
        rest.trim_end_matches("```").trim_end()
    } else {
        s
    };
    if let (Some(start), Some(end)) = (inner.find('['), inner.rfind(']')) {
        inner[start..=end].to_string()
    } else {
        inner.to_string()
    }
}

// ── GET /api/wiki/decisions ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DecisionsQuery {
    pub status: Option<String>,
}

pub async fn list_decisions(
    State(state): State<AppState>,
    Query(q): Query<DecisionsQuery>,
) -> WebResult<Json<serde_json::Value>> {
    let filter = PageFilter {
        kind: Some(PageKind::Decision),
        include_superseded: true,
        ..Default::default()
    };

    let mut pages = state.store.list_pages(&filter)?;

    // Filter by status in memory (avoids dynamic SQL injection).
    if let Some(status) = q.status {
        pages.retain(|_| true); // future: filter by status field
        let _ = status; // placeholder
    }

    Ok(Json(json!({ "decisions": pages })))
}

// ── GET /api/wiki/log ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<usize>,
}

pub async fn get_log(
    State(state): State<AppState>,
    Query(q): Query<LogQuery>,
) -> WebResult<Json<serde_json::Value>> {
    let log_path = WikiPath::new("log.md").map_err(|e| WebError::BadRequest(e.to_string()))?;
    let raw = state.wiki.read_raw(&log_path).map_err(|e| match e {
        bastion_wiki::WikiError::PageNotFound(_) => WebError::NotFound("log.md not found".into()),
        other => WebError::Wiki(other),
    })?;

    // Parse entries: lines starting with `## [`
    let limit = q.limit.unwrap_or(20);
    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| l.starts_with("## ["))
        .take(limit)
        .map(|l| json!({ "heading": l }))
        .collect();

    Ok(Json(json!({ "log": entries, "raw": raw })))
}

// ── GET /api/wiki/pending ─────────────────────────────────────────────────────

pub async fn list_pending(State(state): State<AppState>) -> WebResult<Json<serde_json::Value>> {
    let filter = PageFilter {
        kind: Some(PageKind::ConsolidationProposal),
        ..Default::default()
    };

    let mut proposals = state.wiki.list_pages(&filter)?;
    proposals.retain(|page| {
        let path = page.path.as_str();
        path.starts_with("_pending/") && !path.starts_with("_pending/applied/")
    });

    Ok(Json(json!({ "proposals": proposals })))
}

// ── POST /api/wiki/pending ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProposalBody {
    pub title: String,
    pub pages_affected: Vec<String>,
    pub justification: String,
    pub proposed_changes: String,
}

pub async fn create_proposal(
    State(state): State<AppState>,
    Json(body): Json<CreateProposalBody>,
) -> WebResult<(StatusCode, Json<serde_json::Value>)> {
    use chrono::Local;

    if body.title.trim().is_empty() {
        return Err(WebError::BadRequest("title is required".into()));
    }

    let today = Local::now().format("%Y-%m-%d").to_string();
    let slug = body
        .title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .trim_matches('-')
        .to_string();
    let filename = format!("_pending/consolidation-{today}-{slug}.md");
    let path = WikiPath::new(&filename).map_err(|e| WebError::BadRequest(e.to_string()))?;

    let frontmatter = serde_json::json!({
        "title": body.title,
        "kind": "consolidation-proposal",
        "tier": "working",
        "status": "pending",
        "proposed_at": today,
        "created_at": today,
        "pages_affected": body.pages_affected,
        "pinned": false
    });

    let md_body = format!(
        "## Justificativa\n{}\n\n## Mudanças Propostas\n{}\n",
        body.justification, body.proposed_changes
    );

    state.wiki.write_page(WritePageRequest {
        path: path.clone(),
        frontmatter,
        body: md_body,
        action: CommitAction::Consolidate,
        scope: "_pending".into(),
        subject: format!("propose {slug}"),
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "path": path.as_str(), "status": "pending" })),
    ))
}

// ── GET /api/wiki/search?q=<query> ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search_pages(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> WebResult<Json<serde_json::Value>> {
    if params.q.trim().is_empty() {
        return Err(WebError::BadRequest("q is required".into()));
    }

    let (hits, hybrid) = if let Some(embedder) = state.get_embedder() {
        let query_vec = embedder
            .embed(vec![params.q.clone()])
            .await
            .ok()
            .and_then(|mut v| v.pop());
        let is_hybrid = query_vec.is_some();
        let results = state
            .store
            .hybrid_search(
                &params.q,
                query_vec.as_deref(),
                embedder.provider_id(),
                embedder.model_id(),
                embedder.dimensions(),
                12,
            )
            .unwrap_or_default();
        (results, is_hybrid)
    } else {
        (
            state.store.fts_search(&params.q, 12).unwrap_or_default(),
            false,
        )
    };

    let hits_json: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| json!({ "path": h.path, "title": h.title, "kind": h.kind, "snippet": h.snippet }))
        .collect();

    Ok(Json(
        json!({ "query": params.q, "hits": hits_json, "hybrid": hybrid }),
    ))
}

// ── POST /api/chat/query (SSE streaming) ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChatQueryBody {
    pub query: String,
}

struct WikiContextHit {
    path: String,
    title: String,
    kind: Option<String>,
    snippet: String,
    body: String,
}

async fn retrieve_wiki_context(state: &AppState, query: &str, limit: usize) -> Vec<WikiContextHit> {
    let query_vec = if let Some(embedder) = state.get_embedder() {
        embedder
            .embed(vec![query.to_string()])
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
                query,
                query_vec.as_deref(),
                embedder.provider_id(),
                embedder.model_id(),
                embedder.dimensions(),
                limit,
            )
            .unwrap_or_default()
    } else {
        state.store.fts_search(query, limit).unwrap_or_default()
    };

    hits.into_iter()
        .filter_map(|hit| {
            let path = WikiPath::new(hit.path.clone()).ok()?;
            let page = state.wiki.read_page(&path).ok()?;
            Some(WikiContextHit {
                path: hit.path,
                title: hit.title,
                kind: hit.kind,
                snippet: hit.snippet,
                body: page.body,
            })
        })
        .collect()
}

fn render_wiki_context(hits: &[WikiContextHit]) -> String {
    hits.iter()
        .map(|hit| {
            let body = hit.body.chars().take(4_000).collect::<String>();
            format!(
                "### {} ({})\nKind: {}\nSnippet: {}\n\n{}",
                hit.title,
                hit.path,
                hit.kind.as_deref().unwrap_or("unknown"),
                hit.snippet,
                body,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

pub async fn chat_query(
    State(state): State<AppState>,
    Json(body): Json<ChatQueryBody>,
) -> Result<Response, WebError> {
    let llm = state.get_llm().ok_or(WebError::LlmNotConfigured)?;

    if body.query.trim().is_empty() {
        return Err(WebError::BadRequest("query is required".into()));
    }

    // Build context: read index.md + top hybrid (or FTS) hits.
    let index_raw = state
        .wiki
        .read_raw(&WikiPath::new("index.md").unwrap())
        .unwrap_or_default();

    let wiki_hits = retrieve_wiki_context(&state, &body.query, 5).await;
    let wiki_context = render_wiki_context(&wiki_hits);

    // Inject the user's own paper (target) if it exists in the wiki.
    let target_section = WikiPath::new("papers/my-paper.md")
        .ok()
        .and_then(|p| state.wiki.read_page(&p).ok())
        .map(|page| {
            let body = page.body.chars().take(8_000).collect::<String>();
            format!(
                "\n## Paper do Usuário (Alvo da Análise)\n\
                 Este é o paper que o usuário está escrevendo. \
                 Use os artigos ingeridos na wiki para ajudá-lo a melhorá-lo.\n\n{body}"
            )
        })
        .unwrap_or_default();

    let system = format!(
        "Você é um assistente de pesquisa com acesso à wiki acadêmica Bastion.\n\
         Antes de responder, use as correlações recuperadas da wiki. Responda com base nas informações abaixo e cite as páginas relevantes pelo caminho.\
         {target_section}\n\n\
         ## Índice da Wiki\n{index_raw}\n\n\
         ## Correlações Recuperadas da Wiki\n{wiki_context}"
    );

    let messages = vec![
        Message {
            role: Role::System,
            content: system,
        },
        Message {
            role: Role::User,
            content: body.query.clone(),
        },
    ];

    let provider = llm.provider_id().to_string();
    let model = llm.model_id().to_string();

    let token_stream = llm
        .stream_chat(messages, ChatOptions::default())
        .await
        .map_err(|e| WebError::from_llm_error(e, &provider, &model))?;

    let refs = wiki_hits
        .iter()
        .map(|hit| {
            json!({
                "kind": if hit.kind.as_deref() == Some("paper") { "paper" } else { "wiki" },
                "title": &hit.title,
                "excerpt": &hit.snippet,
                "wiki_path": &hit.path,
            })
        })
        .collect::<Vec<_>>();
    let refs_event = futures_util::stream::once(async move {
        let payload = json!({ "refs": refs });
        Ok::<_, std::convert::Infallible>(format!("event: refs\ndata: {payload}\n\n"))
    });

    // Convert token stream to SSE byte stream.
    let sse_provider = provider.clone();
    let sse_model = model.clone();
    let sse_stream = token_stream.map(move |result| {
        let data = match result {
            Ok(token) => {
                let payload = json!({ "text": token });
                format!("event: token\ndata: {payload}\n\n")
            }
            Err(e) => {
                let error = WebError::from_llm_error(e, &sse_provider, &sse_model);
                let mut payload = json!({ "code": error.code(), "error": error.to_string() });
                if let Some((provider, model, provider_error)) = error.llm_details() {
                    payload["provider"] = json!(provider);
                    payload["model"] = json!(model);
                    payload["provider_error"] = json!(provider_error);
                }
                format!("event: error\ndata: {payload}\n\n")
            }
        };
        Ok::<_, std::convert::Infallible>(data)
    });

    // Append done event.
    let done = futures_util::stream::once(async {
        Ok::<_, std::convert::Infallible>("event: done\ndata: {}\n\n".to_string())
    });
    let full_stream = refs_event.chain(sse_stream).chain(done);

    let body = Body::from_stream(full_stream);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap();

    Ok(resp)
}
