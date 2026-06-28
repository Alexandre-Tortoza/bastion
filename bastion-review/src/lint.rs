//! Wiki integrity lint runner — no LLM required.

use std::collections::HashSet;
use std::sync::Arc;

use bastion_core::{CommitAction, PageFilter, WikiPath, WritePageRequest};
use bastion_store::Store;
use bastion_wiki::{Wiki, extract_links};
use chrono::Local;
use serde_json::json;
use tracing::info;

use crate::error::{ReviewError, ReviewResult};

#[derive(Debug, Clone)]
pub struct LintIssue {
    pub page: String,
    pub kind: &'static str,
    pub detail: String,
}

pub struct LintRunner {
    pub store: Arc<Store>,
    pub wiki: Arc<Wiki>,
}

impl LintRunner {
    pub fn run(&self) -> ReviewResult<(Vec<LintIssue>, String)> {
        let pages = self.store.list_pages(&PageFilter::default())?;
        let mut issues: Vec<LintIssue> = Vec::new();

        // Build set of known paths (without .md) for broken-link checks.
        let known_paths: HashSet<String> = pages
            .iter()
            .map(|m| {
                let s = m.path.as_str();
                s.strip_suffix(".md").unwrap_or(s).to_string()
            })
            .collect();

        for meta in &pages {
            let path_str = meta.path.as_str();

            // missing_field: title or kind empty.
            if meta.title.trim().is_empty() {
                issues.push(LintIssue {
                    page: path_str.to_string(),
                    kind: "missing_field",
                    detail: "campo `title` ausente ou vazio".into(),
                });
            }
            if meta.kind.is_none() {
                issues.push(LintIssue {
                    page: path_str.to_string(),
                    kind: "missing_field",
                    detail: "campo `kind` ausente ou inválido".into(),
                });
            }

            // Read raw to check body + links.
            if let Ok(raw) = self.wiki.read_raw(&meta.path) {
                // empty_body: only whitespace after stripping frontmatter.
                let body = strip_frontmatter(&raw);
                if body.trim().is_empty() {
                    issues.push(LintIssue {
                        page: path_str.to_string(),
                        kind: "empty_body",
                        detail: "body vazio após remoção do frontmatter".into(),
                    });
                }

                // broken_link: wikilinks pointing to nonexistent pages.
                for link in extract_links(&body) {
                    let target = link.path.trim_end_matches(".md");
                    if !known_paths.contains(target) {
                        issues.push(LintIssue {
                            page: path_str.to_string(),
                            kind: "broken_link",
                            detail: format!("`[[{target}]]` não existe"),
                        });
                    }
                }
            }
        }

        let report_path = self.write_report(&issues)?;
        info!(issues = issues.len(), report_path, "lint complete");

        Ok((issues, report_path))
    }

    fn write_report(&self, issues: &[LintIssue]) -> ReviewResult<String> {
        let today = Local::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let file_name = format!("report-{today_str}.md");
        let wiki_path_str = format!("_lint/{file_name}");

        let path = WikiPath::new(&wiki_path_str).map_err(|e| ReviewError::Core(e.to_string()))?;

        let pages_affected: HashSet<&str> = issues.iter().map(|i| i.page.as_str()).collect();

        let summary = format!(
            "{} issue(s) encontrado(s) em {} página(s).",
            issues.len(),
            pages_affected.len()
        );

        let issues_md = if issues.is_empty() {
            "Nenhum problema encontrado.".to_string()
        } else {
            issues
                .iter()
                .map(|i| format!("- **{}** [{}]: {}", i.page, i.kind, i.detail))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let body = format!("## Summary\n{summary}\n\n## Issues\n{issues_md}\n");

        let frontmatter = json!({
            "title": format!("Lint Report {today_str}"),
            "kind": "lint-report",
            "created_at": today_str,
            "updated_at": today_str,
        });

        self.wiki.write_page(WritePageRequest {
            path,
            frontmatter,
            body,
            action: CommitAction::Lint,
            scope: "_lint".into(),
            subject: format!("report-{today_str}"),
        })?;

        Ok(wiki_path_str)
    }
}

fn strip_frontmatter(raw: &str) -> String {
    let s = raw.trim_start();
    if !s.starts_with("---") {
        return raw.to_string();
    }
    // Find closing ---
    let after_open = &s[3..];
    if let Some(pos) = after_open.find("\n---") {
        after_open[pos + 4..].to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_frontmatter_basic() {
        let raw = "---\ntitle: foo\n---\n\nBody here.";
        assert_eq!(strip_frontmatter(raw).trim(), "Body here.");
    }

    #[test]
    fn strip_frontmatter_no_fm() {
        let raw = "Just body";
        assert_eq!(strip_frontmatter(raw), raw);
    }

    #[test]
    fn lint_runner_no_issues_on_clean_wiki() {
        use bastion_wiki::Wiki;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let wiki_path = tmp.path().join("wiki");
        std::fs::create_dir_all(&wiki_path).unwrap();

        let wiki = Wiki::new(&wiki_path, "Test", "test@test.com").unwrap();
        let db_path = tmp.path().join("db.sqlite");
        let store = Store::open(&db_path).unwrap();

        // Write one clean page.
        let page_path = WikiPath::new("concepts/test.md").unwrap();
        wiki.write_page(WritePageRequest {
            path: page_path,
            frontmatter: json!({
                "title": "Test Concept",
                "kind": "concept",
                "tier": "semantic",
                "created_at": "2026-06-27",
                "updated_at": "2026-06-27",
            }),
            body: "## Definition\nA test concept.\n".into(),
            action: CommitAction::Update,
            scope: "concepts".into(),
            subject: "add test".into(),
        })
        .unwrap();

        store.sync_from_wiki(&wiki_path).unwrap();

        let runner = LintRunner {
            store: Arc::new(store),
            wiki: Arc::new(wiki),
        };
        let (issues, _) = runner.run().unwrap();
        // The test page is clean, so no issues for it.
        // (index.md and log.md may have empty body; filter those out.)
        let real_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.page == "concepts/test.md")
            .collect();
        assert!(real_issues.is_empty(), "unexpected issues: {real_issues:?}");
    }
}
