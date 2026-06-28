use std::fmt;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::{BastionError, BastionResult};

// ── WikiPath ──────────────────────────────────────────────────────────────────

/// Validated path relative to the wiki root.
///
/// Valid characters: lowercase ASCII letters, digits, hyphens, underscores,
/// dots, and forward slashes. No leading `/`, no `..` components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WikiPath(String);

impl WikiPath {
    pub fn new(s: impl Into<String>) -> BastionResult<Self> {
        let s = s.into();
        if s.is_empty() {
            return Err(BastionError::InvalidPath("empty path".into()));
        }
        if s.starts_with('/') {
            return Err(BastionError::InvalidPath(format!(
                "path must not start with '/': {s}"
            )));
        }
        for component in s.split('/') {
            if component == ".." {
                return Err(BastionError::InvalidPath(format!(
                    "path must not contain '..': {s}"
                )));
            }
        }
        for c in s.chars() {
            if !matches!(c, 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '/') {
                return Err(BastionError::InvalidPath(format!(
                    "invalid character '{c}' in path: {s}"
                )));
            }
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_markdown(&self) -> bool {
        self.0.ends_with(".md")
    }

    pub fn dir(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(dir, _)| dir)
    }

    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map_or(&self.0, |(_, name)| name)
    }

    pub fn stem(&self) -> &str {
        let name = self.file_name();
        name.strip_suffix(".md").unwrap_or(name)
    }
}

impl fmt::Display for WikiPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ── PageKind ──────────────────────────────────────────────────────────────────

/// Page kind as specified in `docs/wiki.md §3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageKind {
    Paper,
    Concept,
    Method,
    Result,
    Strategy,
    Decision,
    Comparison,
    Synthesis,
    Review,
    ConsolidationProposal,
}

impl PageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Concept => "concept",
            Self::Method => "method",
            Self::Result => "result",
            Self::Strategy => "strategy",
            Self::Decision => "decision",
            Self::Comparison => "comparison",
            Self::Synthesis => "synthesis",
            Self::Review => "review",
            Self::ConsolidationProposal => "consolidation-proposal",
        }
    }

    /// Default tier per `docs/wiki.md §3`.
    pub fn default_tier(self) -> Tier {
        match self {
            Self::Paper | Self::Review => Tier::Episodic,
            Self::ConsolidationProposal => Tier::Working,
            _ => Tier::Semantic,
        }
    }

    /// Canonical wiki subdirectory for this kind.
    pub fn directory(self) -> &'static str {
        match self {
            Self::Paper => "papers",
            Self::Concept => "concepts",
            Self::Method => "methods",
            Self::Result => "results",
            Self::Strategy => "strategies",
            Self::Decision => "decisions",
            Self::Comparison => "comparisons",
            Self::Synthesis => "synthesis",
            Self::Review => "reviews",
            Self::ConsolidationProposal => "_pending",
        }
    }

    /// `index.md` section name for this kind, if it has one.
    pub fn index_section(self) -> Option<&'static str> {
        match self {
            Self::Paper => Some("Papers"),
            Self::Concept => Some("Concepts"),
            Self::Method => Some("Methods"),
            Self::Result => Some("Results"),
            Self::Strategy => Some("Strategies"),
            Self::Decision => Some("Decisions"),
            Self::Comparison => Some("Comparisons"),
            Self::Synthesis => Some("Synthesis"),
            Self::Review => Some("Reviews"),
            Self::ConsolidationProposal => None,
        }
    }
}

impl TryFrom<&str> for PageKind {
    type Error = BastionError;

    fn try_from(s: &str) -> BastionResult<Self> {
        match s {
            "paper" => Ok(Self::Paper),
            "concept" => Ok(Self::Concept),
            "method" => Ok(Self::Method),
            "result" => Ok(Self::Result),
            "strategy" => Ok(Self::Strategy),
            "decision" => Ok(Self::Decision),
            "comparison" => Ok(Self::Comparison),
            "synthesis" => Ok(Self::Synthesis),
            "review" => Ok(Self::Review),
            "consolidation-proposal" => Ok(Self::ConsolidationProposal),
            other => Err(BastionError::InvalidPath(format!(
                "unknown page kind: {other}"
            ))),
        }
    }
}

// ── Tier ─────────────────────────────────────────────────────────────────────

/// Memory tier as specified in `docs/wiki.md §3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Semantic,
    Episodic,
    Working,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Episodic => "episodic",
            Self::Working => "working",
        }
    }
}

impl TryFrom<&str> for Tier {
    type Error = BastionError;

    fn try_from(s: &str) -> BastionResult<Self> {
        match s {
            "semantic" => Ok(Self::Semantic),
            "episodic" => Ok(Self::Episodic),
            "working" => Ok(Self::Working),
            other => Err(BastionError::InvalidPath(format!("unknown tier: {other}"))),
        }
    }
}

// ── CommitAction ──────────────────────────────────────────────────────────────

/// Git commit action — maps to the `action` in `action(scope): subject` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitAction {
    Ingest,
    Update,
    Decision,
    Consolidate,
    Lint,
    Review,
    Delete,
}

impl CommitAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Update => "update",
            Self::Decision => "decision",
            Self::Consolidate => "consolidate",
            Self::Lint => "lint",
            Self::Review => "review",
            Self::Delete => "delete",
        }
    }

    /// Format `action(scope): subject` per `docs/general.md §9`.
    pub fn format_message(self, scope: &str, subject: &str) -> String {
        format!("{}({}): {}", self.as_str(), scope, subject)
    }
}

// ── WikiLink ──────────────────────────────────────────────────────────────────

/// A wikilink extracted from a page body.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WikiLink {
    /// Path relative to wiki root, without leading slash, without `.md`.
    pub path: String,
    /// Display label from `[[path|label]]`.
    pub label: Option<String>,
    /// Section anchor from `[[path#anchor]]`.
    pub anchor: Option<String>,
}

/// A directed wikilink row used by graph visualisations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WikiGraphLink {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub anchor: Option<String>,
}

// ── Page ──────────────────────────────────────────────────────────────────────

/// A fully parsed wiki page.
#[derive(Debug, Clone)]
pub struct Page {
    pub path: WikiPath,
    pub frontmatter: serde_json::Value,
    pub body: String,
    pub links: Vec<WikiLink>,
}

impl Page {
    pub fn kind(&self) -> Option<PageKind> {
        self.frontmatter
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| PageKind::try_from(s).ok())
    }

    pub fn tier(&self) -> Option<Tier> {
        self.frontmatter
            .get("tier")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| Tier::try_from(s).ok())
    }

    pub fn title(&self) -> Option<&str> {
        self.frontmatter
            .get("title")
            .and_then(serde_json::Value::as_str)
    }

    pub fn pinned(&self) -> bool {
        self.frontmatter
            .get("pinned")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }
}

// ── WritePageRequest ──────────────────────────────────────────────────────────

/// Request to write or overwrite a page.
#[derive(Debug, Clone)]
pub struct WritePageRequest {
    pub path: WikiPath,
    pub frontmatter: serde_json::Value,
    pub body: String,
    pub action: CommitAction,
    /// Scope portion of the commit message, e.g. `"papers"`, `"0003"`.
    pub scope: String,
    /// Subject portion of the commit message, e.g. `"add vaswani-2017-attention"`.
    pub subject: String,
}

// ── LogEntry ─────────────────────────────────────────────────────────────────

/// An entry to append to `log.md`.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub date: NaiveDate,
    /// One of: `ingest | update | decision | consolidate | lint | review`.
    pub action: String,
    pub subject: String,
    /// Up to 5 bullet points.
    pub bullets: Vec<String>,
}

impl LogEntry {
    /// Format as a `log.md` section per `docs/wiki.md §6`.
    pub fn format(&self) -> String {
        let mut out = format!("## [{}] {} | {}\n\n", self.date, self.action, self.subject);
        for bullet in self.bullets.iter().take(5) {
            out.push_str(&format!("- {}\n", bullet));
        }
        out.push('\n');
        out
    }
}

// ── PageMeta ─────────────────────────────────────────────────────────────────

/// Minimal page metadata for listing and index.md maintenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub path: WikiPath,
    pub title: String,
    pub kind: Option<PageKind>,
    pub tier: Option<Tier>,
    pub updated_at: Option<String>,
    pub pinned: bool,
}

// ── PageFilter ────────────────────────────────────────────────────────────────

/// Filter options for page listing.
#[derive(Debug, Clone, Default)]
pub struct PageFilter {
    pub kind: Option<PageKind>,
    pub tier: Option<Tier>,
    /// Include pages with `status: superseded` (default: false).
    pub include_superseded: bool,
    pub limit: Option<usize>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_path_valid() {
        assert!(WikiPath::new("papers/vaswani-2017-attention.md").is_ok());
        assert!(WikiPath::new("decisions/0001-no-lora.md").is_ok());
        assert!(WikiPath::new("_pending/consolidation-2026-06-27.md").is_ok());
        assert!(WikiPath::new("index.md").is_ok());
    }

    #[test]
    fn wiki_path_invalid() {
        assert!(WikiPath::new("/papers/foo.md").is_err());
        assert!(WikiPath::new("papers/../secrets.md").is_err());
        assert!(WikiPath::new("Papers/Foo.md").is_err());
        assert!(WikiPath::new("papers/foo bar.md").is_err());
        assert!(WikiPath::new("").is_err());
    }

    #[test]
    fn page_kind_round_trip() {
        for k in [
            PageKind::Paper,
            PageKind::Concept,
            PageKind::Method,
            PageKind::Result,
            PageKind::Strategy,
            PageKind::Decision,
            PageKind::Comparison,
            PageKind::Synthesis,
            PageKind::Review,
            PageKind::ConsolidationProposal,
        ] {
            assert_eq!(PageKind::try_from(k.as_str()).unwrap(), k);
        }
    }

    #[test]
    fn commit_action_format() {
        assert_eq!(
            CommitAction::Ingest.format_message("papers", "add vaswani-2017-attention"),
            "ingest(papers): add vaswani-2017-attention"
        );
        assert_eq!(
            CommitAction::Decision.format_message("0003", "reject lora fine-tuning"),
            "decision(0003): reject lora fine-tuning"
        );
    }

    #[test]
    fn log_entry_format() {
        use chrono::NaiveDate;
        let entry = LogEntry {
            date: NaiveDate::from_ymd_opt(2026, 6, 27).unwrap(),
            action: "ingest".into(),
            subject: "Vaswani 2017".into(),
            bullets: vec!["Criado: [[papers/vaswani-2017-attention]]".into()],
        };
        let s = entry.format();
        assert!(s.starts_with("## [2026-06-27] ingest | Vaswani 2017\n"));
        assert!(s.contains("- Criado:"));
    }
}
