//! Main wiki operations: startup, page CRUD, index.md, log.md, decisions.

use std::path::{Path, PathBuf};

use chrono::Local;
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::atomic::write_atomic;
use crate::error::WikiResult;
use crate::git::{Checkpoint, GitAdapter};
use crate::markdown::{self, Markdown};
use crate::types::{
    CommitAction, LogEntry, Page, PageFilter, PageKind, PageMeta, Tier, WikiPath, WritePageRequest,
};

const WIKI_SUBDIRS: &[&str] = &[
    "papers",
    "concepts",
    "methods",
    "decisions",
    "comparisons",
    "synthesis",
    "reviews",
    "_pending",
    "_lint",
];

/// The wiki filesystem layer.
///
/// Owns the markdown-on-disk source of truth. Every write is atomic and
/// produces an immediate git commit.
#[derive(Clone)]
pub struct Wiki {
    root: PathBuf,
    git: GitAdapter,
}

impl Wiki {
    /// Open or initialise the wiki at `wiki_path`.
    ///
    /// Creates standard subdirectories, initialises the git repo, and writes
    /// `index.md` / `log.md` stubs if they do not exist yet.
    ///
    /// # Errors
    /// Returns [`WikiError`] on IO or git failures.
    pub fn new(wiki_path: &Path, author_name: &str, author_email: &str) -> WikiResult<Self> {
        for subdir in WIKI_SUBDIRS {
            std::fs::create_dir_all(wiki_path.join(subdir))?;
        }

        let git = GitAdapter::open_or_init(wiki_path, author_name, author_email)?;
        let wiki = Self {
            root: wiki_path.to_path_buf(),
            git,
        };

        wiki.ensure_index_md()?;
        wiki.ensure_log_md()?;

        info!(root = %wiki_path.display(), "wiki initialised");
        Ok(wiki)
    }

    /// Absolute filesystem path for a wiki-relative path.
    fn fs_path(&self, path: &WikiPath) -> PathBuf {
        self.root.join(path.as_str())
    }

    fn ensure_index_md(&self) -> WikiResult<()> {
        let path = self.root.join("index.md");
        if path.exists() {
            return Ok(());
        }
        let today = Local::now().format("%Y-%m-%d");
        let content = format!(
            "# Wiki Index\n\
             Last updated: {today} | Total pages: 0\n\
             \n\
             ## Papers (0)\n\
             \n\
             ## Concepts (0)\n\
             \n\
             ## Methods (0)\n\
             \n\
             ## Decisions (0)\n\
             \n\
             ## Comparisons (0)\n\
             \n\
             ## Synthesis (0)\n\
             \n\
             ## Reviews (0)\n"
        );
        write_atomic(&path, content.as_bytes())?;
        debug!("created index.md stub");
        Ok(())
    }

    fn ensure_log_md(&self) -> WikiResult<()> {
        let path = self.root.join("log.md");
        if path.exists() {
            return Ok(());
        }
        write_atomic(&path, b"# Wiki Log\n\n")?;
        debug!("created log.md stub");
        Ok(())
    }

    // ── Read operations ──────────────────────────────────────────────────────

    /// Read and parse a page by its wiki-root-relative path.
    ///
    /// # Errors
    /// Returns [`WikiError::PageNotFound`] if the file does not exist.
    pub fn read_page(&self, path: &WikiPath) -> WikiResult<Page> {
        let fs = self.fs_path(path);
        if !fs.exists() {
            return Err(crate::error::WikiError::PageNotFound(path.to_string()));
        }
        let text = std::fs::read_to_string(&fs)?;
        let md = markdown::parse(&text)?;
        let links = markdown::extract_links(&md.body);
        Ok(Page {
            path: path.clone(),
            frontmatter: md.frontmatter,
            body: md.body,
            links,
        })
    }

    /// Read a page as raw text without parsing.
    ///
    /// # Errors
    /// Returns [`WikiError::PageNotFound`] if the file does not exist.
    pub fn read_raw(&self, path: &WikiPath) -> WikiResult<String> {
        let fs = self.fs_path(path);
        if !fs.exists() {
            return Err(crate::error::WikiError::PageNotFound(path.to_string()));
        }
        Ok(std::fs::read_to_string(&fs)?)
    }

    /// Returns `true` if a page exists at the given path.
    pub fn page_exists(&self, path: &WikiPath) -> bool {
        self.fs_path(path).exists()
    }

    /// List pages matching `filter`.
    ///
    /// Skips `index.md`, `log.md`, `_lint/`, and `_pending/` by default.
    ///
    /// # Errors
    /// Propagates IO errors from directory traversal or file reads.
    pub fn list_pages(&self, filter: &PageFilter) -> WikiResult<Vec<PageMeta>> {
        let mut results = Vec::new();

        for entry in WalkDir::new(&self.root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file() && e.path().extension().map_or(false, |ext| ext == "md")
            })
        {
            let rel = entry
                .path()
                .strip_prefix(&self.root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");

            // Skip top-level special files and hidden directories unless pending proposals are requested.
            if !rel.contains('/')
                || rel.starts_with("_lint/")
                || rel.starts_with("_pending/")
                    && filter.kind != Some(PageKind::ConsolidationProposal)
            {
                continue;
            }

            let wiki_path = match WikiPath::new(&rel) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let text = match std::fs::read_to_string(entry.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let md = match markdown::parse(&text) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let kind = md
                .frontmatter
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| PageKind::try_from(s).ok());

            let tier = md
                .frontmatter
                .get("tier")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| Tier::try_from(s).ok());

            if let Some(fk) = filter.kind {
                if kind != Some(fk) {
                    continue;
                }
            }
            if let Some(ft) = filter.tier {
                if tier != Some(ft) {
                    continue;
                }
            }
            if !filter.include_superseded {
                let status = md
                    .frontmatter
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                if status == "superseded" {
                    continue;
                }
            }

            let title = markdown::derive_title(&md.frontmatter, &md.body, &rel);
            let updated_at = md
                .frontmatter
                .get("updated_at")
                .and_then(serde_json::Value::as_str)
                .map(String::from);
            let pinned = md
                .frontmatter
                .get("pinned")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);

            results.push(PageMeta {
                path: wiki_path,
                title,
                kind,
                tier,
                updated_at,
                pinned,
            });
        }

        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    // ── Write operations ──────────────────────────────────────────────────────

    /// Write (create or overwrite) a page, then commit to git.
    ///
    /// Always sets `updated_at` to today's date before writing.
    ///
    /// # Errors
    /// Propagates IO, YAML, and git errors.
    pub fn write_page(&self, req: WritePageRequest) -> WikiResult<()> {
        let mut fm = req.frontmatter;
        if !fm.is_object() {
            fm = serde_json::json!({});
        }
        let today = Local::now().format("%Y-%m-%d").to_string();
        fm.as_object_mut()
            .unwrap()
            .insert("updated_at".into(), serde_json::Value::String(today));

        let text = markdown::emit(&Markdown {
            frontmatter: fm,
            body: req.body,
        })?;
        write_atomic(&self.fs_path(&req.path), text.as_bytes())?;

        let message = req.action.format_message(&req.scope, &req.subject);
        self.git.commit_all(&message)?;

        info!(path = %req.path, "wrote wiki page");
        Ok(())
    }

    // ── index.md management ──────────────────────────────────────────────────

    /// Update or insert the entry for `meta` in `index.md`.
    ///
    /// Finds the correct section by kind, removes any existing entry for the
    /// same path, inserts the new line, and rewrites section counts.
    ///
    /// Kinds without a section (`ConsolidationProposal`) are silently skipped.
    ///
    /// # Errors
    /// Propagates IO errors.
    pub fn update_index_entry(&self, meta: &PageMeta) -> WikiResult<()> {
        let section_name = match meta.kind.and_then(PageKind::index_section) {
            Some(s) => s,
            None => return Ok(()),
        };

        let index_path = self.root.join("index.md");
        let existing = if index_path.exists() {
            std::fs::read_to_string(&index_path)?
        } else {
            return Ok(());
        };

        let today = Local::now().format("%Y-%m-%d").to_string();
        let kind_str = meta.kind.map(PageKind::as_str).unwrap_or("unknown");
        let updated = meta.updated_at.as_deref().unwrap_or(&today);
        let path_no_md = meta
            .path
            .as_str()
            .strip_suffix(".md")
            .unwrap_or(meta.path.as_str());

        let new_line = format!(
            "- [[{}|{}]] — {} ({}, {})",
            path_no_md, meta.title, meta.title, kind_str, updated
        );

        // Remove existing entry for this path.
        let path_key = format!("[[{}", path_no_md);
        let filtered: Vec<&str> = existing
            .lines()
            .filter(|l| !l.contains(&path_key))
            .collect();

        // Insert after the matching section header.
        let mut new_lines: Vec<String> = Vec::with_capacity(filtered.len() + 2);
        let mut inserted = false;
        let mut i = 0;
        while i < filtered.len() {
            let line = filtered[i];
            new_lines.push(line.to_string());
            if !inserted && section_header_matches(line, section_name) {
                // Skip the blank line immediately after the header, if any.
                if filtered.get(i + 1).map_or(false, |l| l.is_empty()) {
                    new_lines.push(String::new());
                    i += 1;
                }
                new_lines.push(new_line.clone());
                inserted = true;
            }
            i += 1;
        }
        if !inserted {
            new_lines.push(String::new());
            new_lines.push(format!("## {} (0)", section_name));
            new_lines.push(String::new());
            new_lines.push(new_line);
        }

        let content = rebuild_index_counts(&new_lines.join("\n"), &today);
        write_atomic(&index_path, content.as_bytes())?;
        Ok(())
    }

    // ── log.md management ────────────────────────────────────────────────────

    /// Prepend a log entry to `log.md` (newest entries at top).
    ///
    /// # Errors
    /// Propagates IO errors.
    pub fn append_log(&self, entry: &LogEntry) -> WikiResult<()> {
        let log_path = self.root.join("log.md");
        let existing = if log_path.exists() {
            std::fs::read_to_string(&log_path)?
        } else {
            "# Wiki Log\n\n".to_string()
        };

        let entry_text = entry.format();

        // Insert after the first blank line following the header.
        let new_content = if let Some(pos) = existing.find("\n\n") {
            let (header, rest) = existing.split_at(pos + 2);
            format!("{}{}{}", header, entry_text, rest)
        } else {
            format!("{}\n{}", existing.trim_end(), entry_text)
        };

        write_atomic(&log_path, new_content.as_bytes())?;
        Ok(())
    }

    // ── Decisions ────────────────────────────────────────────────────────────

    /// Returns the next sequential decision number.
    ///
    /// Scans `decisions/` for files matching the `NNNN-*.md` pattern and
    /// returns `max_found + 1`, starting at `1` when the directory is empty.
    ///
    /// # Errors
    /// Propagates IO errors.
    pub fn next_decision_number(&self) -> WikiResult<u32> {
        let dir = self.root.join("decisions");
        if !dir.exists() {
            return Ok(1);
        }
        let mut max = 0u32;
        for entry in std::fs::read_dir(&dir)? {
            let name = entry?.file_name();
            let name = name.to_string_lossy();
            if let Some(num_str) = name.split('-').next() {
                if let Ok(n) = num_str.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
        Ok(max + 1)
    }

    // ── Wikilinks ────────────────────────────────────────────────────────────

    /// Resolve a wikilink path string to a [`WikiPath`] if the page exists.
    ///
    /// Accepts paths with or without `.md` extension.
    #[must_use]
    pub fn resolve_wikilink(&self, path_str: &str) -> Option<WikiPath> {
        let normalized = if path_str.ends_with(".md") {
            path_str.to_string()
        } else {
            format!("{}.md", path_str)
        };
        let path = WikiPath::new(normalized).ok()?;
        if self.page_exists(&path) {
            Some(path)
        } else {
            None
        }
    }

    // ── Delete ───────────────────────────────────────────────────────────────

    /// Delete a page from the wiki filesystem and commit the removal.
    ///
    /// # Errors
    /// Returns [`WikiError::PageNotFound`] if the file does not exist.
    pub fn delete_page(&self, path: &WikiPath) -> WikiResult<()> {
        let fs = self.fs_path(path);
        if !fs.exists() {
            return Err(crate::error::WikiError::PageNotFound(path.to_string()));
        }
        std::fs::remove_file(&fs)?;
        self.remove_index_entry(path)?;
        let scope = path.dir().unwrap_or("wiki").to_string();
        let subject = format!("remove {}", path.stem());
        let message = CommitAction::Delete.format_message(&scope, &subject);
        self.git.commit_all(&message)?;
        info!(path = %path, "deleted wiki page");
        Ok(())
    }

    /// Remove a page's entry from `index.md` and rewrite section counts.
    fn remove_index_entry(&self, path: &WikiPath) -> WikiResult<()> {
        let index_path = self.root.join("index.md");
        if !index_path.exists() {
            return Ok(());
        }
        let existing = std::fs::read_to_string(&index_path)?;
        let path_no_md = path.as_str().strip_suffix(".md").unwrap_or(path.as_str());
        let path_key = format!("[[{}", path_no_md);
        if !existing.lines().any(|l| l.contains(&path_key)) {
            return Ok(());
        }
        let filtered: Vec<&str> = existing
            .lines()
            .filter(|l| !l.contains(&path_key))
            .collect();
        let content = format!("{}\n", filtered.join("\n"));
        let today = Local::now().format("%Y-%m-%d").to_string();
        let content = rebuild_index_counts(&content, &today);
        write_atomic(&index_path, content.as_bytes())?;
        Ok(())
    }

    // ── Git ──────────────────────────────────────────────────────────────────

    /// Return recent git checkpoints.
    ///
    /// # Errors
    /// Propagates libgit2 errors.
    pub fn recent_checkpoints(&self, limit: usize) -> WikiResult<Vec<Checkpoint>> {
        self.git.recent_checkpoints(limit)
    }

    /// Wiki root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn section_header_matches(line: &str, name: &str) -> bool {
    let Some(rest) = line.strip_prefix("## ") else {
        return false;
    };
    rest == name || rest.starts_with(&format!("{} (", name))
}

fn count_entries_in_section(content: &str, section_name: &str) -> usize {
    let mut in_section = false;
    let mut count = 0;
    for line in content.lines() {
        if section_header_matches(line, section_name) {
            in_section = true;
            continue;
        }
        if in_section {
            if line.starts_with("## ") {
                in_section = false;
            } else if line.starts_with("- [[") {
                count += 1;
            }
        }
    }
    count
}

fn rebuild_index_counts(content: &str, today: &str) -> String {
    let papers = count_entries_in_section(content, "Papers");
    let concepts = count_entries_in_section(content, "Concepts");
    let methods = count_entries_in_section(content, "Methods");
    let decisions = count_entries_in_section(content, "Decisions");
    let comparisons = count_entries_in_section(content, "Comparisons");
    let synthesis = count_entries_in_section(content, "Synthesis");
    let reviews = count_entries_in_section(content, "Reviews");
    let total = papers + concepts + methods + decisions + comparisons + synthesis + reviews;

    let rebuilt: Vec<String> = content
        .lines()
        .map(|line| {
            if line.starts_with("Last updated:") {
                format!("Last updated: {} | Total pages: {}", today, total)
            } else if section_header_matches(line, "Papers") {
                format!("## Papers ({})", papers)
            } else if section_header_matches(line, "Concepts") {
                format!("## Concepts ({})", concepts)
            } else if section_header_matches(line, "Methods") {
                format!("## Methods ({})", methods)
            } else if section_header_matches(line, "Decisions") {
                format!("## Decisions ({})", decisions)
            } else if section_header_matches(line, "Comparisons") {
                format!("## Comparisons ({})", comparisons)
            } else if section_header_matches(line, "Synthesis") {
                format!("## Synthesis ({})", synthesis)
            } else if section_header_matches(line, "Reviews") {
                format!("## Reviews ({})", reviews)
            } else {
                line.to_string()
            }
        })
        .collect();

    format!("{}\n", rebuilt.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use tempfile::TempDir;

    fn make_wiki(dir: &Path) -> Wiki {
        Wiki::new(dir, "Bastion", "bastion@local").unwrap()
    }

    #[test]
    fn new_creates_subdirectories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        make_wiki(&root);

        for subdir in WIKI_SUBDIRS {
            assert!(root.join(subdir).is_dir(), "missing subdir: {subdir}");
        }
        assert!(root.join("index.md").is_file());
        assert!(root.join("log.md").is_file());
    }

    #[test]
    fn new_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        make_wiki(&root);
        // Second call must not fail or overwrite existing files.
        make_wiki(&root);
    }

    #[test]
    fn write_and_read_page() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));
        let path = WikiPath::new("concepts/self-attention.md").unwrap();

        let fm = serde_json::json!({
            "title": "Self-Attention",
            "kind": "concept",
            "tier": "semantic",
            "created_at": "2026-06-27",
            "pinned": false,
            "tags": []
        });

        wiki.write_page(WritePageRequest {
            path: path.clone(),
            frontmatter: fm,
            body: "## Definition\nA mechanism that...\n".into(),
            action: crate::types::CommitAction::Ingest,
            scope: "concepts".into(),
            subject: "add self-attention".into(),
        })
        .unwrap();

        let page = wiki.read_page(&path).unwrap();
        assert_eq!(page.frontmatter["title"], "Self-Attention");
        assert_eq!(page.frontmatter["kind"], "concept");
        assert!(page.frontmatter["updated_at"].as_str().is_some());
    }

    #[test]
    fn page_exists_reflects_reality() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));
        let path = WikiPath::new("concepts/x.md").unwrap();
        assert!(!wiki.page_exists(&path));

        wiki.write_page(WritePageRequest {
            path: path.clone(),
            frontmatter: serde_json::json!({ "title": "X" }),
            body: "body\n".into(),
            action: crate::types::CommitAction::Update,
            scope: "concepts".into(),
            subject: "add x".into(),
        })
        .unwrap();

        assert!(wiki.page_exists(&path));
    }

    #[test]
    fn next_decision_number_starts_at_1() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));
        assert_eq!(wiki.next_decision_number().unwrap(), 1);
    }

    #[test]
    fn next_decision_number_increments() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        for (n, slug) in [(1, "first"), (2, "second"), (3, "third")] {
            let path = WikiPath::new(format!("decisions/{:04}-{}.md", n, slug)).unwrap();
            wiki.write_page(WritePageRequest {
                path,
                frontmatter: serde_json::json!({ "title": slug, "kind": "decision" }),
                body: "body\n".into(),
                action: crate::types::CommitAction::Decision,
                scope: format!("{:04}", n),
                subject: slug.into(),
            })
            .unwrap();
        }

        assert_eq!(wiki.next_decision_number().unwrap(), 4);
    }

    #[test]
    fn append_log_prepends_entry() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        let entry = LogEntry {
            date: NaiveDate::from_ymd_opt(2026, 6, 27).unwrap(),
            action: "ingest".into(),
            subject: "Vaswani 2017".into(),
            bullets: vec!["Criado: [[papers/vaswani-2017-attention]]".into()],
        };
        wiki.append_log(&entry).unwrap();

        let log = std::fs::read_to_string(wiki.root().join("log.md")).unwrap();
        assert!(log.contains("## [2026-06-27] ingest | Vaswani 2017"));
        assert!(log.contains("- Criado:"));
    }

    #[test]
    fn update_index_entry_adds_to_section() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        let meta = PageMeta {
            path: WikiPath::new("papers/vaswani-2017-attention.md").unwrap(),
            title: "Attention Is All You Need".into(),
            kind: Some(PageKind::Paper),
            tier: Some(Tier::Episodic),
            updated_at: Some("2026-06-27".into()),
            pinned: false,
        };
        wiki.update_index_entry(&meta).unwrap();

        let index = std::fs::read_to_string(wiki.root().join("index.md")).unwrap();
        assert!(index.contains("[[papers/vaswani-2017-attention|Attention Is All You Need]]"));
        assert!(index.contains("## Papers (1)"));
        assert!(index.contains("Total pages: 1"));
    }

    #[test]
    fn resolve_wikilink_found_and_missing() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        let path = WikiPath::new("concepts/x.md").unwrap();
        wiki.write_page(WritePageRequest {
            path,
            frontmatter: serde_json::json!({ "title": "X" }),
            body: "body\n".into(),
            action: crate::types::CommitAction::Update,
            scope: "concepts".into(),
            subject: "add x".into(),
        })
        .unwrap();

        assert!(wiki.resolve_wikilink("concepts/x").is_some());
        assert!(wiki.resolve_wikilink("concepts/x.md").is_some());
        assert!(wiki.resolve_wikilink("concepts/missing").is_none());
    }

    #[test]
    fn list_pages_filter_by_kind() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        wiki.write_page(WritePageRequest {
            path: WikiPath::new("papers/foo.md").unwrap(),
            frontmatter: serde_json::json!({ "title": "Foo", "kind": "paper" }),
            body: "body\n".into(),
            action: crate::types::CommitAction::Ingest,
            scope: "papers".into(),
            subject: "add foo".into(),
        })
        .unwrap();
        wiki.write_page(WritePageRequest {
            path: WikiPath::new("concepts/bar.md").unwrap(),
            frontmatter: serde_json::json!({ "title": "Bar", "kind": "concept" }),
            body: "body\n".into(),
            action: crate::types::CommitAction::Update,
            scope: "concepts".into(),
            subject: "add bar".into(),
        })
        .unwrap();

        let papers = wiki
            .list_pages(&PageFilter {
                kind: Some(PageKind::Paper),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].title, "Foo");
    }

    #[test]
    fn git_checkpoints_recorded_on_write() {
        let tmp = TempDir::new().unwrap();
        let wiki = make_wiki(&tmp.path().join("wiki"));

        wiki.write_page(WritePageRequest {
            path: WikiPath::new("concepts/test.md").unwrap(),
            frontmatter: serde_json::json!({ "title": "Test" }),
            body: "body\n".into(),
            action: crate::types::CommitAction::Update,
            scope: "concepts".into(),
            subject: "add test".into(),
        })
        .unwrap();

        let checkpoints = wiki.recent_checkpoints(5).unwrap();
        assert!(!checkpoints.is_empty());
        assert!(checkpoints[0].summary.contains("update(concepts)"));
    }
}
