//! Bastion wiki filesystem layer.
//!
//! Owns the markdown-on-disk source of truth: atomic writes, frontmatter
//! parsing and emission, wikilink extraction, and git versioning.

mod atomic;
mod error;
mod git;
mod markdown;
mod types;
mod wiki;

pub use error::{WikiError, WikiResult};
pub use git::{Checkpoint, GitAdapter};
pub use markdown::{Markdown, derive_title, emit, extract_links, parse};
pub use types::{
    CommitAction, LogEntry, Page, PageFilter, PageKind, PageMeta, Tier, WikiLink, WikiPath,
    WritePageRequest,
};
pub use wiki::Wiki;
