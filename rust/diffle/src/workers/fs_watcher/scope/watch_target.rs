//! What a watcher is pointed at.

use std::path::{self, Path, PathBuf};
use std::sync::Arc;

use crate::workers::fs_watcher::error::WatchError;

/// Whether a checkout path is ignored. It receives the event path relative to the checkout
/// root, POSIX-separated, without a leading `./`, and answers true when that path or any
/// ancestor is ignored. It is consulted per event, so its owner may change its answers
/// while the watcher runs.
pub type IgnoreRule = Arc<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Clone)]
pub enum WatchTarget {
    /// The checkout plus Git metadata (HEAD, refs, packed-refs, index). `git_dir` may lie
    /// inside `root` (`.git`), outside it (a linked worktree), or be relative to the
    /// current directory.
    Worktree {
        root: PathBuf,
        git_dir: PathBuf,
        common_dir: PathBuf,
        ignored: IgnoreRule,
    },
    /// Only the Git metadata that moves the old side of a diff (HEAD, refs, packed-refs):
    /// no checkout, no index.
    Refs {
        git_dir: PathBuf,
        common_dir: PathBuf,
    },
}

impl WatchTarget {
    #[must_use]
    pub fn git_dir(&self) -> &Path {
        match self {
            Self::Worktree { git_dir, .. } | Self::Refs { git_dir, .. } => git_dir,
        }
    }

    #[must_use]
    pub fn common_dir(&self) -> &Path {
        match self {
            Self::Worktree { common_dir, .. } | Self::Refs { common_dir, .. } => common_dir,
        }
    }

    /// The checkout root and its ignore rule, when a checkout is watched.
    #[must_use]
    pub fn checkout(&self) -> Option<(&Path, &IgnoreRule)> {
        match self {
            Self::Worktree { root, ignored, .. } => Some((root, ignored)),
            Self::Refs { .. } => None,
        }
    }

    /// The same target with every path made absolute against the current directory, so
    /// the paths the OS reports back compare equal to the ones held here.
    pub fn absolute(self) -> Result<Self, WatchError> {
        match self {
            Self::Worktree {
                root,
                git_dir,
                common_dir,
                ignored,
            } => Ok(Self::Worktree {
                root: absolute(root)?,
                git_dir: absolute(git_dir)?,
                common_dir: absolute(common_dir)?,
                ignored,
            }),
            Self::Refs {
                git_dir,
                common_dir,
            } => Ok(Self::Refs {
                git_dir: absolute(git_dir)?,
                common_dir: absolute(common_dir)?,
            }),
        }
    }
}

fn absolute(path: PathBuf) -> Result<PathBuf, WatchError> {
    path::absolute(&path).map_err(|source| WatchError::Resolve { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(git_dir: &str, common_dir: &str) -> WatchTarget {
        WatchTarget::Refs {
            git_dir: git_dir.into(),
            common_dir: common_dir.into(),
        }
    }

    #[test]
    fn absolute_anchors_relative_paths_and_keeps_absolute_ones() {
        let target = refs("rel/.git", "/abs/.git").absolute().unwrap();
        assert!(target.git_dir().is_absolute());
        assert!(target.git_dir().ends_with("rel/.git"));
        assert_eq!(target.common_dir(), Path::new("/abs/.git"));
    }

    #[test]
    fn only_a_worktree_target_has_a_checkout() {
        assert!(refs("/g", "/g").checkout().is_none());
        let worktree = WatchTarget::Worktree {
            root: "/r".into(),
            git_dir: "/r/.git".into(),
            common_dir: "/r/.git".into(),
            ignored: Arc::new(|_| false),
        };
        let (root, _) = worktree.checkout().unwrap();
        assert_eq!(root, Path::new("/r"));
        assert_eq!(worktree.git_dir(), Path::new("/r/.git"));
    }
}
