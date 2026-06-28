//! Bastion core types, traits, and errors.
//!
//! Zero IO — every type here is a pure value or error variant.

mod error;
mod types;

pub use error::{BastionError, BastionResult};
pub use types::{
    CommitAction, LogEntry, Page, PageFilter, PageKind, PageMeta, Tier, WikiGraphLink, WikiLink,
    WikiPath, WritePageRequest,
};
