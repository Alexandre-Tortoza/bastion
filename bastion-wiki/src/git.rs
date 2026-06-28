//! Git versioning for the wiki tree.
//!
//! Thin wrapper around `git2`. Opens or initialises a repo at the wiki root;
//! every write produces an immediate commit. Author identity is configurable
//! via `BASTION_GIT_AUTHOR_NAME` / `BASTION_GIT_AUTHOR_EMAIL`. Adapted from
//! ai-memory-wiki.

use std::path::{Path, PathBuf};

use git2::{IndexAddOption, ObjectType, Repository, Signature};
use tracing::{debug, warn};

use crate::error::{WikiError, WikiResult};

/// A single git checkpoint in the wiki repository.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub oid: String,
    pub summary: String,
    /// Author timestamp, seconds since Unix epoch.
    pub time: i64,
}

/// Thin handle over the wiki git repository.
#[derive(Clone)]
pub struct GitAdapter {
    root: PathBuf,
    author_name: String,
    author_email: String,
}

impl GitAdapter {
    /// Open or initialise the git repo at `root`. Idempotent.
    ///
    /// # Errors
    /// Propagates libgit2 errors.
    pub fn open_or_init(root: &Path, author_name: &str, author_email: &str) -> WikiResult<Self> {
        std::fs::create_dir_all(root)?;
        match Repository::open(root) {
            Ok(_) => debug!(root = %root.display(), "wiki repo already initialised"),
            Err(_) => {
                debug!(root = %root.display(), "initialising wiki repo");
                Repository::init(root).map_err(map_git_err)?;
            }
        }
        Ok(Self {
            root: root.to_path_buf(),
            author_name: author_name.to_string(),
            author_email: author_email.to_string(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stage everything in the wiki root and commit with `message`.
    ///
    /// Returns `Ok(None)` when the working tree is clean (nothing to commit),
    /// or `Ok(Some(oid))` on success.
    ///
    /// # Errors
    /// Propagates libgit2 errors.
    pub fn commit_all(&self, message: &str) -> WikiResult<Option<git2::Oid>> {
        let repo = Repository::open(&self.root).map_err(map_git_err)?;

        let mut index = repo.index().map_err(map_git_err)?;
        index
            .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
            .map_err(map_git_err)?;
        index.write().map_err(map_git_err)?;

        let tree_oid = index.write_tree().map_err(map_git_err)?;

        // Skip commit if nothing changed relative to HEAD.
        if let Ok(head) = repo.head()
            && let Some(target) = head.target()
            && let Ok(parent_commit) = repo.find_commit(target)
            && parent_commit.tree_id() == tree_oid
        {
            debug!("working tree clean; no commit");
            return Ok(None);
        }
        // Fresh repo with empty index: also skip.
        if repo.head().is_err() && index.is_empty() {
            debug!("fresh repo, empty index; no commit");
            return Ok(None);
        }

        let tree = repo.find_tree(tree_oid).map_err(map_git_err)?;
        let sig = Signature::now(&self.author_name, &self.author_email).map_err(map_git_err)?;

        let parents: Vec<git2::Commit<'_>> = match repo.head() {
            Ok(head) => match head.target() {
                Some(oid) => vec![repo.find_commit(oid).map_err(map_git_err)?],
                None => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .map_err(map_git_err)?;
        debug!(oid = %oid, "wiki commit");
        Ok(Some(oid))
    }

    /// Count commits reachable from HEAD. Returns 0 for an empty repo.
    #[must_use]
    pub fn commit_count(&self) -> usize {
        let Ok(repo) = Repository::open(&self.root) else {
            return 0;
        };
        let Ok(mut walk) = repo.revwalk() else {
            return 0;
        };
        if walk.push_head().is_err() {
            return 0;
        }
        walk.count()
    }

    /// Return the most recent commits reachable from HEAD.
    ///
    /// # Errors
    /// Propagates libgit2 errors.
    pub fn recent_checkpoints(&self, limit: usize) -> WikiResult<Vec<Checkpoint>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let repo = Repository::open(&self.root).map_err(map_git_err)?;
        let mut walk = repo.revwalk().map_err(map_git_err)?;
        if walk.push_head().is_err() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(limit.min(100));
        for oid in walk.take(limit) {
            let oid = oid.map_err(map_git_err)?;
            let commit = repo.find_commit(oid).map_err(map_git_err)?;
            out.push(Checkpoint {
                oid: oid.to_string(),
                summary: commit.summary().unwrap_or("(no summary)").to_string(),
                time: commit.time().seconds(),
            });
        }
        Ok(out)
    }

    /// Read `path` (relative to wiki root) as it existed at `rev`.
    ///
    /// # Errors
    /// Returns [`WikiError`] when the revision, path, or blob cannot be found.
    pub fn file_at_rev(&self, rev: &str, path: &Path) -> WikiResult<Vec<u8>> {
        let repo = Repository::open(&self.root).map_err(map_git_err)?;
        let object = repo.revparse_single(rev).map_err(map_git_err)?;
        let commit = object.peel_to_commit().map_err(map_git_err)?;
        let tree = commit.tree().map_err(map_git_err)?;
        let entry = tree.get_path(path).map_err(map_git_err)?;
        let blob = entry
            .to_object(&repo)
            .map_err(map_git_err)?
            .peel(ObjectType::Blob)
            .map_err(map_git_err)?;
        let blob = blob
            .as_blob()
            .ok_or_else(|| WikiError::Git(format!("{} at {rev} is not a file", path.display())))?;
        Ok(blob.content().to_vec())
    }
}

fn map_git_err(e: git2::Error) -> WikiError {
    warn!(error = %e, "libgit2 error");
    WikiError::Git(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn adapter(dir: &Path) -> GitAdapter {
        GitAdapter::open_or_init(dir, "Bastion", "bastion@local").unwrap()
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        adapter(&root);
        assert!(root.join(".git").is_dir());
        // Second init is a no-op.
        adapter(&root);
    }

    #[test]
    fn commit_all_returns_none_when_clean() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        let git = adapter(&root);
        assert!(git.commit_all("initial").unwrap().is_none());

        std::fs::write(root.join("foo.md"), "hello").unwrap();
        let oid = git.commit_all("add foo").unwrap();
        assert!(oid.is_some());

        // No changes: None again.
        assert!(git.commit_all("no changes").unwrap().is_none());
        assert_eq!(git.commit_count(), 1);
    }

    #[test]
    fn commit_all_captures_deletes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        let git = adapter(&root);
        std::fs::write(root.join("a.md"), "first").unwrap();
        git.commit_all("first").unwrap();
        std::fs::remove_file(root.join("a.md")).unwrap();
        assert!(git.commit_all("remove a").unwrap().is_some());
        assert_eq!(git.commit_count(), 2);
    }

    #[test]
    fn recent_checkpoints_newest_first() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        let git = adapter(&root);

        std::fs::write(root.join("a.md"), "one").unwrap();
        let first = git.commit_all("first checkpoint").unwrap().unwrap();
        std::fs::write(root.join("a.md"), "two").unwrap();
        let second = git.commit_all("second checkpoint").unwrap().unwrap();

        let checkpoints = git.recent_checkpoints(10).unwrap();
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints[0].oid, second.to_string());
        assert_eq!(checkpoints[0].summary, "second checkpoint");
        assert_eq!(checkpoints[1].oid, first.to_string());
    }

    #[test]
    fn file_at_rev_reads_historical_blob() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("wiki");
        let git = adapter(&root);

        std::fs::write(root.join("a.md"), "one").unwrap();
        let first = git.commit_all("first").unwrap().unwrap();
        std::fs::write(root.join("a.md"), "two").unwrap();
        git.commit_all("second").unwrap();

        let bytes = git
            .file_at_rev(&first.to_string(), Path::new("a.md"))
            .unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(), "one");
    }
}
