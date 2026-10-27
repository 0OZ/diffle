//! Detached checkout of an owned clone, and where its review comments live.
use std::path::{Path, PathBuf};

use super::GitRepo;
use super::exec::ExecOptions;
use crate::error::{GitError, Result};

/// Object ids `rev-parse` prints: SHA-1, or SHA-256 when the repository uses it.
const SHA1_OBJECT_ID_LEN: usize = 40;
const SHA256_OBJECT_ID_LEN: usize = 64;

impl GitRepo {
    /// Whether `clone_into` created this repository. `open` is never isolated.
    pub fn is_isolated(&self) -> bool {
        self.isolated
    }

    /// Directory `CommentStore::open` treats as a git dir: it appends `diffle`.
    pub fn review_state_dir(&self) -> &Path {
        &self.review_state_dir
    }

    /// Point review comments at a durable pseudo-git-root. Git objects stay in `git_dir`.
    pub fn set_review_state_dir(&mut self, dir: PathBuf) {
        self.review_state_dir = dir;
    }

    /// Stop the batch reader and wait until that process has exited.
    pub async fn close(&self) {
        self.cat_file.close().await;
    }

    /// Check out `sha` detached inside this clone.
    ///
    /// An ordinary repository is refused before git runs. `sha` is resolved to a commit, and only
    /// that full hex object id is passed to checkout. Uncommitted changes in the clone are
    /// discarded so the worktree matches the commit. If the command fails and HEAD moved, HEAD is
    /// restored to the previous commit the same way; the original error is returned, and a failed
    /// restore is reported together with it.
    pub async fn checkout_owned(&self, sha: &str) -> Result<()> {
        if !self.isolated {
            return Err(GitError::refused("refusing to check out an ordinary repository", &["checkout"]).into());
        }
        let oid = self.resolve(sha).await?;
        if !is_full_object_id(&oid) {
            return Err(GitError::refused("checkout target is not a commit", &["checkout"]).into());
        }
        let before = self.resolve("HEAD").await?;
        if let Err(error) = self.detach(&oid).await {
            return self.fail_checkout(&before, error).await;
        }
        match self.resolve("HEAD").await {
            Ok(after) if after == oid => Ok(()),
            Ok(after) => {
                self.fail_checkout(
                    &before,
                    GitError::refused(format!("checkout left HEAD at {after}, not {oid}"), &["checkout"]),
                )
                .await
            }
            Err(error) => self.fail_checkout(&before, error).await,
        }
    }

    async fn detach(&self, oid: &str) -> Result<(), GitError> {
        self.exec(&["checkout", "--detach", "--force", "--quiet", oid], ExecOptions::default()).await?;
        Ok(())
    }

    /// `checkout_err` is always returned. A HEAD that moved is put back first.
    async fn fail_checkout(&self, before: &str, checkout_err: GitError) -> Result<()> {
        let unchanged = self.resolve("HEAD").await.ok().as_deref() == Some(before);
        if unchanged {
            return Err(checkout_err.into());
        }
        if !is_full_object_id(before) {
            return Err(GitError::refused(
                format!("checkout failed: {checkout_err}; refusing to restore HEAD from {before}"),
                &["checkout"],
            )
            .into());
        }
        if let Err(restore) = self.detach(before).await {
            return Err(GitError::refused(
                format!("checkout failed: {checkout_err}; restoring HEAD failed: {restore}"),
                &["checkout"],
            )
            .into());
        }
        Err(checkout_err.into())
    }
}

fn is_full_object_id(sha: &str) -> bool {
    matches!(sha.len(), SHA1_OBJECT_ID_LEN | SHA256_OBJECT_ID_LEN)
        && sha.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use crate::git::owned_paths;

    #[test]
    fn ownership_validation_refuses_missing_paths() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("missing");
        assert!(owned_paths(root.path(), root.path(), &missing, root.path()).is_err());
    }

    // Symlinks are the reason lexical containment cannot grant mutation authority.
    #[cfg(unix)]
    #[test]
    fn ownership_validation_refuses_each_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let git = root.path().join(".git");
        std::fs::create_dir(&git).unwrap();
        let escape = root.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &escape).unwrap();
        for (name, opened_root, git_dir, common_dir) in [
            ("root", escape.as_path(), git.as_path(), git.as_path()),
            ("git_dir", root.path(), escape.as_path(), git.as_path()),
            ("common_dir", root.path(), git.as_path(), escape.as_path()),
        ] {
            assert!(
                owned_paths(root.path(), opened_root, git_dir, common_dir).is_err(),
                "{name} escaped through a symlink but was granted ownership"
            );
        }
    }
}
