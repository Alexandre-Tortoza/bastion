// Re-export all domain types from bastion-core.
// bastion-wiki modules continue to use `crate::types::*` without change.
pub use bastion_core::{
    CommitAction, LogEntry, Page, PageFilter, PageKind, PageMeta, Tier, WikiLink, WikiPath,
    WritePageRequest,
};
