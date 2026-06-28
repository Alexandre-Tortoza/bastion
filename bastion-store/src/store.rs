//! The `Store` struct: open, index, search, embeddings.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use bastion_core::{PageFilter, PageKind, PageMeta, Tier, WikiGraphLink, WikiLink, WikiPath};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::error::StoreResult;
use crate::migrations;

/// An FTS5 search result.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub kind: Option<String>,
    pub snippet: String,
}

/// The SQLite store. Mutex-wrapped connection; safe to share via `Arc<Store>`.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (or create) the SQLite database at `db_path` and run migrations.
    ///
    /// Enables WAL mode and `foreign_keys = ON`.
    pub fn open(db_path: &Path) -> StoreResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )?;
        migrations::run(&mut conn)?;
        info!(db = %db_path.display(), "store opened");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ── Write ─────────────────────────────────────────────────────────────────

    pub fn upsert_page(&self, meta: &PageMeta, body: &str, mtime: u64) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        upsert_page_conn(&conn, meta, body, "{}", None, mtime)?;
        debug!(path = meta.path.as_str(), "upserted page");
        Ok(())
    }

    pub fn upsert_page_full(
        &self,
        meta: &PageMeta,
        body: &str,
        frontmatter_json: &str,
        status: Option<&str>,
        mtime: u64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        upsert_page_conn(&conn, meta, body, frontmatter_json, status, mtime)
    }

    pub fn delete_page(&self, path: &str) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        let page_id: Option<i64> = conn
            .query_row("SELECT id FROM pages WHERE path = ?1", [path], |r| r.get(0))
            .optional()?;
        if let Some(id) = page_id {
            conn.execute("DELETE FROM embeddings WHERE page_id = ?1", [id])?;
        }
        conn.execute(
            "DELETE FROM page_links WHERE source_path = ?1 OR target_path = ?1",
            [path],
        )?;
        conn.execute("DELETE FROM pages WHERE path = ?1", [path])?;
        debug!(path, "deleted page from store");
        Ok(())
    }

    pub fn update_links(&self, source_path: &str, links: &[WikiLink]) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM page_links WHERE source_path = ?1",
            [source_path],
        )?;
        for link in links {
            conn.execute(
                "INSERT OR IGNORE INTO page_links (source_path, target_path, label, anchor)
                 VALUES (?1, ?2, ?3, ?4)",
                params![source_path, link.path, link.label, link.anchor],
            )?;
        }
        Ok(())
    }

    // ── Read ──────────────────────────────────────────────────────────────────

    pub fn fts_search(&self, query: &str, limit: usize) -> StoreResult<Vec<SearchHit>> {
        let conn = self.conn.lock().unwrap();
        // Wrap in double quotes for phrase matching; escape embedded quotes.
        let fts_query = format!("\"{}\"", query.replace('"', "\"\""));
        let mut stmt = conn.prepare(
            "SELECT p.path, p.title, p.kind,
                    snippet(pages_fts, 1, '<b>', '</b>', '…', 20) AS snippet
             FROM pages_fts
             JOIN pages p ON pages_fts.rowid = p.id
             WHERE pages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                Ok(SearchHit {
                    path: row.get(0)?,
                    title: row.get(1)?,
                    kind: row.get(2)?,
                    snippet: row.get(3)?,
                })
            })?
            .filter_map(|r| r.map_err(|e| warn!(error = %e, "FTS result error")).ok())
            .collect();
        Ok(hits)
    }

    pub fn get_page_meta(&self, path: &str) -> StoreResult<Option<PageMeta>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, title, kind, tier, updated_at, pinned
             FROM pages WHERE path = ?1",
        )?;
        let result = stmt
            .query_row([path], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })
            .optional()?;

        Ok(
            result.and_then(|(path, title, kind, tier, updated_at, pinned)| {
                let path = WikiPath::new(path).ok()?;
                Some(PageMeta {
                    path,
                    title,
                    kind: kind.as_deref().and_then(|s| PageKind::try_from(s).ok()),
                    tier: tier.as_deref().and_then(|s| Tier::try_from(s).ok()),
                    updated_at,
                    pinned,
                })
            }),
        )
    }

    pub fn list_pages(&self, filter: &PageFilter) -> StoreResult<Vec<PageMeta>> {
        let conn = self.conn.lock().unwrap();
        let kind_clause = filter
            .kind
            .map(|k| format!(" AND kind = '{}'", k.as_str()))
            .unwrap_or_default();
        let tier_clause = filter
            .tier
            .map(|t| format!(" AND tier = '{}'", t.as_str()))
            .unwrap_or_default();
        let superseded_clause = if filter.include_superseded {
            ""
        } else {
            " AND (status IS NULL OR status != 'superseded')"
        };
        let limit_clause = filter
            .limit
            .map(|n| format!(" LIMIT {}", n))
            .unwrap_or_default();

        let sql = format!(
            "SELECT path, title, kind, tier, updated_at, pinned
             FROM pages
             WHERE path NOT LIKE '\\_pending/%' ESCAPE '\\'
               AND path NOT LIKE '\\_lint/%' ESCAPE '\\'
               AND path NOT IN ('index.md', 'log.md')
               {kind_clause}{tier_clause}{superseded_clause}
             ORDER BY updated_at DESC{limit_clause}"
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(path, title, kind, tier, updated_at, pinned)| {
                let path = WikiPath::new(path).ok()?;
                Some(PageMeta {
                    path,
                    title,
                    kind: kind.as_deref().and_then(|s| PageKind::try_from(s).ok()),
                    tier: tier.as_deref().and_then(|s| Tier::try_from(s).ok()),
                    updated_at,
                    pinned,
                })
            })
            .collect();
        Ok(rows)
    }

    pub fn backlinks(&self, target_path: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT source_path FROM page_links WHERE target_path = ?1 ORDER BY source_path",
        )?;
        let paths = stmt
            .query_map([target_path], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(paths)
    }

    pub fn get_all_links(&self) -> StoreResult<Vec<WikiGraphLink>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT source_path, target_path, label, anchor
             FROM page_links
             ORDER BY source_path, target_path",
        )?;
        let links = stmt
            .query_map([], |row| {
                Ok(WikiGraphLink {
                    source: row.get(0)?,
                    target: row.get(1)?,
                    label: row.get(2)?,
                    anchor: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(links)
    }

    // ── Embeddings ───────────────────────────────────────────────────────────

    /// Store or update a page embedding. Computes SHA-256 of `body` to track staleness.
    pub fn upsert_embedding(
        &self,
        path: &str,
        provider: &str,
        model: &str,
        dim: usize,
        vector: &[f32],
        body: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        let page_id: Option<i64> = conn
            .query_row("SELECT id FROM pages WHERE path = ?1", [path], |r| r.get(0))
            .optional()?;

        let page_id = match page_id {
            Some(id) => id,
            None => {
                warn!(path, "upsert_embedding: page not found in index, skipping");
                return Ok(());
            }
        };

        let blob = vec_to_blob(vector);
        let sha = body_sha(body);

        conn.execute(
            "INSERT INTO embeddings (page_id, provider, model, dim, vector, content_sha)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(page_id, provider, model) DO UPDATE SET
               dim         = excluded.dim,
               vector      = excluded.vector,
               content_sha = excluded.content_sha",
            params![page_id, provider, model, dim as i64, blob, sha],
        )?;
        debug!(path, provider, model, "embedding upserted");
        Ok(())
    }

    /// Return pages whose embedding is absent or whose body has changed since last embed.
    ///
    /// Returns `(page_id, path, body)` triples ready for re-embedding.
    pub fn pages_needing_embed(
        &self,
        provider: &str,
        model: &str,
    ) -> StoreResult<Vec<(i64, String, String)>> {
        // Acquire the lock once for the entire operation.
        let conn = self.conn.lock().unwrap();

        // Fetch pages with their stored content_sha via LEFT JOIN.
        // Rows where e.page_id IS NULL have no embedding yet.
        let mut stmt = conn.prepare(
            "SELECT p.id, p.path, p.body, e.content_sha
             FROM pages p
             LEFT JOIN embeddings e ON e.page_id = p.id
                 AND e.provider = ?1
                 AND e.model    = ?2
             WHERE p.path NOT LIKE '\\_pending/%' ESCAPE '\\'
               AND p.path NOT LIKE '\\_lint/%' ESCAPE '\\'
               AND p.path NOT IN ('index.md', 'log.md')",
        )?;

        let needs: Vec<(i64, String, String)> = stmt
            .query_map(params![provider, model], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter(|(_, _, body, stored_sha)| {
                // Include row if no embedding stored, or body SHA has changed.
                match stored_sha {
                    None => true,
                    Some(sha) => sha != &body_sha(body),
                }
            })
            .map(|(id, path, body, _)| (id, path, body))
            .collect();

        Ok(needs)
    }

    /// Semantic search: rank pages by dot-product similarity to `query_vec`.
    ///
    /// Returns `(path, score)` sorted descending. OpenAI/Voyage vectors are
    /// already L2-normalised so dot product equals cosine similarity.
    pub fn cosine_search(
        &self,
        query_vec: &[f32],
        provider: &str,
        model: &str,
        dim: usize,
        limit: usize,
    ) -> StoreResult<Vec<(String, f32)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.path, e.vector
             FROM embeddings e
             JOIN pages p ON p.id = e.page_id
             WHERE e.provider = ?1 AND e.model = ?2 AND e.dim = ?3",
        )?;

        let mut scored: Vec<(String, f32)> = stmt
            .query_map(params![provider, model, dim as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .filter_map(|r| r.ok())
            .map(|(path, blob)| {
                let score = dot_product(query_vec, &blob);
                (path, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        Ok(scored)
    }

    /// Hybrid search combining FTS5 and cosine similarity via Reciprocal Rank Fusion (k=60).
    ///
    /// `query_vec = None` falls back to FTS5-only results.
    pub fn hybrid_search(
        &self,
        fts_query: &str,
        query_vec: Option<&[f32]>,
        provider: &str,
        model: &str,
        dim: usize,
        limit: usize,
    ) -> StoreResult<Vec<SearchHit>> {
        let pool = limit * 4;

        let fts = self.fts_search(fts_query, pool)?;
        let sem: Vec<(String, f32)> = match query_vec {
            Some(v) => self.cosine_search(v, provider, model, dim, pool)?,
            None => vec![],
        };

        // RRF: score(d) = Σ 1/(60 + rank)
        let mut scores: HashMap<String, f64> = HashMap::new();
        for (rank, hit) in fts.iter().enumerate() {
            *scores.entry(hit.path.clone()).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
        }
        for (rank, (path, _)) in sem.iter().enumerate() {
            *scores.entry(path.clone()).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
        }

        // Build a snippet index from FTS results.
        let snippet_idx: HashMap<_, _> = fts.iter().map(|h| (h.path.clone(), h)).collect();

        // Sort paths by RRF score, take top-N.
        let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(limit);

        let hits = ranked
            .into_iter()
            .filter_map(|(path, _)| {
                // Reuse FTS hit if available; otherwise do a bare metadata lookup.
                if let Some(h) = snippet_idx.get(&path) {
                    Some(SearchHit {
                        path: h.path.clone(),
                        title: h.title.clone(),
                        kind: h.kind.clone(),
                        snippet: h.snippet.clone(),
                    })
                } else {
                    // Semantic-only hit: pull metadata from pages table.
                    let conn = self.conn.lock().unwrap();
                    conn.query_row(
                        "SELECT path, title, kind FROM pages WHERE path = ?1",
                        [&path],
                        |row| {
                            Ok(SearchHit {
                                path: row.get(0)?,
                                title: row.get(1)?,
                                kind: row.get(2)?,
                                snippet: String::new(),
                            })
                        },
                    )
                    .optional()
                    .ok()
                    .flatten()
                }
            })
            .collect();

        Ok(hits)
    }

    // ── Sync ─────────────────────────────────────────────────────────────────

    /// Scan `wiki_root` and re-index pages whose mtime is newer than what's stored.
    ///
    /// Acquires the lock once for the entire scan to avoid repeated locking and
    /// to prevent double-lock from index_file → upsert_page_full.
    pub fn sync_from_wiki(&self, wiki_root: &Path) -> StoreResult<usize> {
        use walkdir::WalkDir;

        let conn = self.conn.lock().unwrap();
        let mut count = 0usize;

        for entry in WalkDir::new(wiki_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file() && e.path().extension().map_or(false, |ext| ext == "md")
            })
        {
            let rel = match entry.path().strip_prefix(wiki_root) {
                Ok(r) => r.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            if !rel.contains('/') {
                continue;
            }

            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let stored_mtime: Option<i64> = conn
                .query_row(
                    "SELECT indexed_mtime FROM pages WHERE path = ?1",
                    [&rel],
                    |row| row.get(0),
                )
                .optional()
                .unwrap_or(None);

            if stored_mtime == Some(mtime as i64) {
                continue;
            }

            match index_file_with_conn(&conn, wiki_root, &rel, mtime) {
                Ok(_) => count += 1,
                Err(e) => warn!(path = %rel, error = %e, "failed to index page"),
            }
        }

        if count > 0 {
            info!(count, "sync_from_wiki: re-indexed pages");
        }
        Ok(count)
    }
}

// ── Vector helpers ────────────────────────────────────────────────────────────

/// Serialize a f32 slice to little-endian bytes for SQLite BLOB storage.
fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Dot product between a query vector and a stored BLOB (little-endian f32 chunks).
///
/// For L2-normalised vectors (OpenAI, Voyage) this equals cosine similarity.
fn dot_product(query: &[f32], blob: &[u8]) -> f32 {
    query
        .iter()
        .zip(blob.chunks_exact(4))
        .map(|(q, c)| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) * q)
        .sum()
}

/// SHA-256 hex digest of a page body. Used to detect stale embeddings.
fn body_sha(body: &str) -> String {
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    hex::encode(h.finalize())
}

// ── Free helpers (take &Connection to avoid double-locking) ──────────────────

fn upsert_page_conn(
    conn: &Connection,
    meta: &PageMeta,
    body: &str,
    frontmatter_json: &str,
    status: Option<&str>,
    mtime: u64,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO pages (path, title, kind, tier, updated_at, pinned, status, frontmatter, body, indexed_mtime)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(path) DO UPDATE SET
           title         = excluded.title,
           kind          = excluded.kind,
           tier          = excluded.tier,
           updated_at    = excluded.updated_at,
           pinned        = excluded.pinned,
           status        = excluded.status,
           frontmatter   = excluded.frontmatter,
           body          = excluded.body,
           indexed_mtime = excluded.indexed_mtime",
        params![
            meta.path.as_str(),
            meta.title,
            meta.kind.map(PageKind::as_str),
            meta.tier.map(Tier::as_str),
            meta.updated_at,
            meta.pinned as i64,
            status,
            frontmatter_json,
            body,
            mtime as i64,
        ],
    )?;
    Ok(())
}

fn index_file_with_conn(
    conn: &Connection,
    wiki_root: &Path,
    rel: &str,
    mtime: u64,
) -> StoreResult<()> {
    let text = std::fs::read_to_string(wiki_root.join(rel))?;
    let (frontmatter_json, title, kind, tier, updated_at, pinned, status, body) =
        parse_page_for_index(&text);

    let path = match WikiPath::new(rel) {
        Ok(p) => p,
        Err(e) => {
            warn!(path = rel, error = %e, "invalid wiki path, skipping");
            return Ok(());
        }
    };

    let meta = PageMeta {
        path: path.clone(),
        title,
        kind,
        tier,
        updated_at,
        pinned,
    };
    upsert_page_conn(
        conn,
        &meta,
        &body,
        &frontmatter_json,
        status.as_deref(),
        mtime,
    )?;

    let links = extract_wikilinks(&body);
    conn.execute(
        "DELETE FROM page_links WHERE source_path = ?1",
        [path.as_str()],
    )?;
    for link in &links {
        conn.execute(
            "INSERT OR IGNORE INTO page_links (source_path, target_path, label, anchor) \
             VALUES (?1, ?2, ?3, ?4)",
            params![path.as_str(), link.path, link.label, link.anchor],
        )?;
    }

    Ok(())
}

fn extract_wikilinks(body: &str) -> Vec<WikiLink> {
    use std::collections::BTreeSet;
    let mut seen: BTreeSet<(String, Option<String>, Option<String>)> = BTreeSet::new();
    let mut in_fence = false;

    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("[[") {
            let after_open = &rest[start + 2..];
            let Some(close) = after_open.find("]]") else {
                break;
            };
            let raw = &after_open[..close];
            let (path_and_anchor, label) = if let Some((p, l)) = raw.split_once('|') {
                (p.trim(), Some(l.trim().to_string()))
            } else {
                (raw.trim(), None)
            };
            let (path, anchor) = if let Some((p, a)) = path_and_anchor.split_once('#') {
                (p.trim(), Some(a.trim().to_string()))
            } else {
                (path_and_anchor, None)
            };
            if !path.is_empty() && !path.contains("://") {
                seen.insert((path.to_string(), label, anchor));
            }
            rest = &after_open[close + 2..];
        }
    }

    seen.into_iter()
        .map(|(path, label, anchor)| WikiLink { path, label, anchor })
        .collect()
}

fn parse_page_for_index(
    text: &str,
) -> (
    String,
    String,
    Option<PageKind>,
    Option<Tier>,
    Option<String>,
    bool,
    Option<String>,
    String,
) {
    let trimmed = text.strip_prefix('\u{FEFF}').unwrap_or(text);

    let (fm_str, body) = if let Some(rest) = trimmed.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            (&rest[..end], rest[end + 5..].to_string())
        } else {
            ("", text.to_string())
        }
    } else {
        ("", text.to_string())
    };

    if fm_str.is_empty() {
        return (
            "{}".into(),
            String::new(),
            None,
            None,
            None,
            false,
            None,
            body,
        );
    }

    let fm: serde_json::Value = serde_yaml::from_str(fm_str)
        .ok()
        .and_then(|v: serde_yaml::Value| serde_json::to_value(v).ok())
        .unwrap_or(serde_json::json!({}));

    let fm_json = fm.to_string();

    let title = fm
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .unwrap_or_default();

    let kind = fm
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| PageKind::try_from(s).ok());

    let tier = fm
        .get("tier")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| Tier::try_from(s).ok());

    let updated_at = fm
        .get("updated_at")
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    let pinned = fm
        .get("pinned")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let status = fm
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(String::from);

    (fm_json, title, kind, tier, updated_at, pinned, status, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_store(dir: &Path) -> Store {
        Store::open(&dir.join("test.sqlite")).unwrap()
    }

    fn sample_meta(path: &str) -> PageMeta {
        PageMeta {
            path: WikiPath::new(path).unwrap(),
            title: "Test Page".into(),
            kind: Some(PageKind::Concept),
            tier: Some(Tier::Semantic),
            updated_at: Some("2026-06-27".into()),
            pinned: false,
        }
    }

    #[test]
    fn open_creates_db_and_runs_migrations() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let db = store.conn.lock().unwrap();
        let mut stmt = db
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"pages".into()));
        assert!(tables.contains(&"page_links".into()));
        assert!(tables.contains(&"embeddings".into()));
    }

    #[test]
    fn upsert_and_get_page_meta() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let meta = sample_meta("concepts/self-attention.md");
        store.upsert_page(&meta, "body text", 12345).unwrap();

        let got = store
            .get_page_meta("concepts/self-attention.md")
            .unwrap()
            .unwrap();
        assert_eq!(got.title, "Test Page");
        assert_eq!(got.kind, Some(PageKind::Concept));
    }

    #[test]
    fn upsert_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let meta = sample_meta("concepts/x.md");
        store.upsert_page(&meta, "first body", 1).unwrap();
        let mut meta2 = meta.clone();
        meta2.title = "Updated".into();
        store.upsert_page(&meta2, "second body", 2).unwrap();
        let got = store.get_page_meta("concepts/x.md").unwrap().unwrap();
        assert_eq!(got.title, "Updated");
    }

    #[test]
    fn fts_search_finds_body_match() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let meta = sample_meta("concepts/transformer.md");
        store
            .upsert_page(&meta, "Transformers use self-attention mechanisms.", 0)
            .unwrap();
        let hits = store.fts_search("self-attention", 10).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "concepts/transformer.md");
    }

    #[test]
    fn update_links_and_backlinks() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let meta = sample_meta("papers/foo.md");
        store.upsert_page(&meta, "body", 0).unwrap();

        let links = vec![
            WikiLink {
                path: "concepts/bar".into(),
                label: None,
                anchor: None,
            },
            WikiLink {
                path: "methods/baz".into(),
                label: Some("Baz Method".into()),
                anchor: None,
            },
        ];
        store.update_links("papers/foo.md", &links).unwrap();

        let backlinks = store.backlinks("concepts/bar").unwrap();
        assert_eq!(backlinks, vec!["papers/foo.md"]);
    }

    #[test]
    fn list_pages_filter_by_kind() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());

        let paper_meta = PageMeta {
            path: WikiPath::new("papers/foo.md").unwrap(),
            title: "Foo".into(),
            kind: Some(PageKind::Paper),
            tier: Some(Tier::Episodic),
            updated_at: None,
            pinned: false,
        };
        store.upsert_page(&paper_meta, "body", 0).unwrap();
        store
            .upsert_page(&sample_meta("concepts/bar.md"), "body", 0)
            .unwrap();

        let papers = store
            .list_pages(&PageFilter {
                kind: Some(PageKind::Paper),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].title, "Foo");
    }

    #[test]
    fn sync_from_wiki_indexes_new_files() {
        let tmp = TempDir::new().unwrap();
        let wiki_root = tmp.path().join("wiki");
        let concepts = wiki_root.join("concepts");
        std::fs::create_dir_all(&concepts).unwrap();

        let content = "---\ntitle: Self-Attention\nkind: concept\ntier: semantic\n---\n\
                       Self-attention is a mechanism.\n";
        std::fs::write(concepts.join("self-attention.md"), content).unwrap();

        let store = open_store(tmp.path());
        let count = store.sync_from_wiki(&wiki_root).unwrap();
        assert_eq!(count, 1);

        let meta = store.get_page_meta("concepts/self-attention.md").unwrap();
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().title, "Self-Attention");

        let count2 = store.sync_from_wiki(&wiki_root).unwrap();
        assert_eq!(count2, 0);
    }

    #[test]
    fn upsert_and_cosine_search() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());

        // Create three pages with distinct synthetic vectors.
        let pages = vec![
            ("concepts/alpha.md", vec![1.0f32, 0.0, 0.0]),
            ("concepts/beta.md", vec![0.0f32, 1.0, 0.0]),
            ("concepts/gamma.md", vec![0.0f32, 0.0, 1.0]),
        ];
        for (path, _) in &pages {
            let meta = PageMeta {
                path: WikiPath::new(*path).unwrap(),
                title: path.to_string(),
                kind: Some(PageKind::Concept),
                tier: None,
                updated_at: None,
                pinned: false,
            };
            store.upsert_page(&meta, "body text", 0).unwrap();
        }
        for (path, vec) in &pages {
            store
                .upsert_embedding(path, "test", "test-model", 3, vec, "body text")
                .unwrap();
        }

        // Query closest to alpha.
        let results = store
            .cosine_search(&[1.0, 0.0, 0.0], "test", "test-model", 3, 3)
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "concepts/alpha.md");
        assert!((results[0].1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hybrid_search_no_embedder() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());
        let meta = sample_meta("concepts/attention.md");
        store
            .upsert_page(&meta, "self-attention mechanism in transformers", 0)
            .unwrap();

        // Without embedder (query_vec = None) should fall back to FTS only.
        let hits = store
            .hybrid_search("attention", None, "test", "model", 3, 10)
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].path, "concepts/attention.md");
    }

    #[test]
    fn pages_needing_embed_detects_stale() {
        let tmp = TempDir::new().unwrap();
        let store = open_store(tmp.path());

        let meta = sample_meta("concepts/stale.md");
        store.upsert_page(&meta, "original body", 0).unwrap();
        store
            .upsert_embedding(
                "concepts/stale.md",
                "openai",
                "text-embedding-3-small",
                3,
                &[1.0, 0.0, 0.0],
                "original body",
            )
            .unwrap();

        // Embedding is fresh → should not appear.
        let needs = store
            .pages_needing_embed("openai", "text-embedding-3-small")
            .unwrap();
        assert!(needs.iter().all(|(_, p, _)| p != "concepts/stale.md"));

        // Update body → embedding is now stale.
        let meta2 = PageMeta {
            title: "Updated".into(),
            ..meta
        };
        store.upsert_page(&meta2, "updated body", 1).unwrap();

        let needs2 = store
            .pages_needing_embed("openai", "text-embedding-3-small")
            .unwrap();
        assert!(needs2.iter().any(|(_, p, _)| p == "concepts/stale.md"));
    }
}
