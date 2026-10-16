//! The Git metadata a review depends on, and where to listen for it.
//!
//! HEAD and refs move the old side in every live mode. In a worktree the index also decides
//! the tree and the changed set (`git add -f`, `git rm --cached`). Git replaces these files
//! atomically (write a `.lock`, rename over), which can orphan a watch on the file itself,
//! so the listening is on their directories and the matching is by path.

use std::path::{Path, PathBuf};

use super::watch_target::WatchTarget;

const HEAD: &str = "HEAD";
const PACKED_REFS: &str = "packed-refs";
const INDEX: &str = "index";
const REFS: &str = "refs";

#[derive(Debug, Clone)]
pub struct MetadataPaths {
    git_dirs: Vec<PathBuf>,
    files: Vec<PathBuf>,
    refs_dir: PathBuf,
}

impl MetadataPaths {
    #[must_use]
    pub fn of(target: &WatchTarget) -> Self {
        let (git_dir, common_dir) = (target.git_dir(), target.common_dir());
        let mut git_dirs = vec![git_dir.to_path_buf()];
        let mut files = vec![git_dir.join(HEAD)];
        if common_dir != git_dir {
            git_dirs.push(common_dir.to_path_buf());
            files.push(common_dir.join(HEAD));
        }
        files.push(common_dir.join(PACKED_REFS));
        if target.checkout().is_some() {
            files.push(git_dir.join(INDEX));
        }
        Self {
            git_dirs,
            files,
            refs_dir: common_dir.join(REFS),
        }
    }

    /// The Git directories (the per-worktree one and the shared one, once if they are the
    /// same) whose direct children are listened to for HEAD, packed-refs and the index.
    #[must_use]
    pub fn git_dirs(&self) -> &[PathBuf] {
        &self.git_dirs
    }

    /// The directory of every branch, tag and remote ref, listened to recursively.
    #[must_use]
    pub fn refs_dir(&self) -> &Path {
        &self.refs_dir
    }

    /// Whether `path` lies in one of the Git directories.
    #[must_use]
    pub fn is_inside_git(&self, path: &Path) -> bool {
        self.git_dirs.iter().any(|dir| path.starts_with(dir))
    }

    /// Whether `path` is one of the metadata files, or a ref (or the refs directory).
    #[must_use]
    pub fn is_metadata(&self, path: &Path) -> bool {
        self.files.iter().any(|file| file == path) || path.starts_with(&self.refs_dir)
    }
}

#[cfg(test)]
#[path = "metadata_paths_tests.rs"]
mod tests;
