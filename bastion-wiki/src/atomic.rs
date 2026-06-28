//! Atomic file writes via tmp + rename + fsync.
//!
//! A crash mid-write never produces a torn file. Adapted from ai-memory-wiki.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::{WikiError, WikiResult};

/// Atomically replace the file at `path` with `bytes`.
///
/// Writes to a tempfile in the same directory, syncs, renames over the
/// destination, then best-effort syncs the parent directory.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> WikiResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| WikiError::Io(std::io::Error::other("path has no parent")))?;
    std::fs::create_dir_all(parent)?;

    let mut tmp = tempfile::Builder::new()
        .prefix(".bastion-tmp.")
        .tempfile_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_data()?;

    let persisted: File = tmp.persist(path)?;
    persisted.sync_data()?;

    if let Ok(dir) = File::open(parent) {
        let _ = dir.sync_all();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_atomically_and_creates_parents() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("nested/dir/page.md");
        write_atomic(&target, b"hello").unwrap();
        assert!(target.is_file());
        assert_eq!(std::fs::read(&target).unwrap(), b"hello");
    }

    #[test]
    fn overwrites_existing_file() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("page.md");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
    }

    #[test]
    fn does_not_leave_tmp_files_on_success() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("page.md");
        write_atomic(&target, b"x").unwrap();
        let leftover = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .any(|n| n.to_string_lossy().starts_with(".bastion-tmp."));
        assert!(!leftover, "tempfile leaked");
    }
}
