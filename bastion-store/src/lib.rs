//! Bastion SQLite store — derived index over the markdown wiki.
//!
//! The store is never the source of truth. Markdown files on disk are.
//! All writes go through the wiki; the store is rebuilt from disk on startup
//! and updated via `upsert_page` on every wiki write.

mod error;
mod migrations;
mod store;

pub use error::{StoreError, StoreResult};
pub use store::{SearchHit, Store};
