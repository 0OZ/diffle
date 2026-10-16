//! One consistent view of the review at a version: which files changed, what the new side
//! holds, and the commits the comparison spans.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::repository::history::commit_info::CommitInfo;
use crate::domains::repository::history::neighbours::Neighbours;
use crate::domains::repository::history::range_commits::RangeCommits;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;
use crate::domains::settings::context_lines::ContextLines;
use crate::kernel::side::Side;

/// The commit under review when the comparison is a single commit, with its neighbours along
/// `HEAD`'s first-parent history for stepping through it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedCommit {
    pub info: CommitInfo,
    pub neighbours: Neighbours,
}

/// The commits a comparison spans, and the endpoints they were listed between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedCommits {
    pub commits: RangeCommits,
    pub old: TreeSource,
    pub new: TreeSource,
}

/// Where the compared refs resolve now, when that differs from the shown snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The snapshot a reload would supersede.
    pub version: SnapshotVersion,
    pub old: TreeSource,
    pub new: TreeSource,
}

/// Everything a snapshot is made of; [`Snapshot::new`] indexes it for reading.
#[derive(Debug, Clone)]
pub struct SnapshotParts {
    pub root: PathBuf,
    pub mode: ReviewMode,
    pub version: SnapshotVersion,
    pub old: TreeSource,
    pub new: TreeSource,
    /// The checked-out commit; `None` on an unborn branch.
    pub head: Option<ObjectId>,
    pub context: ContextLines,
    pub changed: Vec<ChangedFile>,
    /// Every path on the new side.
    pub tree: BTreeSet<RepoPath>,
    pub commit: Option<ReviewedCommit>,
    pub commits: SpannedCommits,
}

/// Immutable once built. The new-side `tree` is the allowlist for every file read.
#[derive(Debug, Clone)]
pub struct Snapshot {
    parts: SnapshotParts,
    changed_by_path: HashMap<RepoPath, usize>,
    rename_sources: HashSet<RepoPath>,
}

impl Snapshot {
    #[must_use]
    pub fn new(parts: SnapshotParts) -> Self {
        let changed_by_path = parts
            .changed
            .iter()
            .enumerate()
            .map(|(at, file)| (file.path.clone(), at))
            .collect();
        let rename_sources = parts
            .changed
            .iter()
            .filter_map(|file| file.kind.from_path().cloned())
            .collect();
        Self {
            parts,
            changed_by_path,
            rename_sources,
        }
    }

    #[must_use]
    pub const fn parts(&self) -> &SnapshotParts {
        &self.parts
    }

    #[must_use]
    pub const fn version(&self) -> SnapshotVersion {
        self.parts.version
    }

    #[must_use]
    pub fn changed_file(&self, path: &RepoPath) -> Option<&ChangedFile> {
        self.changed_by_path
            .get(path)
            .and_then(|at| self.parts.changed.get(*at))
    }

    /// The source a side is read from.
    #[must_use]
    pub const fn source(&self, side: Side) -> &TreeSource {
        match side {
            Side::Old => &self.parts.old,
            Side::New => &self.parts.new,
        }
    }

    /// The path to read on `side` for a snapshot path, or `None` when the snapshot does not
    /// expose it. New side: tree paths only. Old side: a changed file's old path (a rename
    /// maps new to old), plus tree paths and rename sources. Ignored files, `.git` and
    /// traversal are never exposed.
    #[must_use]
    pub fn side_path(&self, path: &RepoPath, side: Side) -> Option<RepoPath> {
        match side {
            Side::New => self.parts.tree.contains(path).then(|| path.clone()),
            Side::Old => match self.changed_file(path) {
                Some(file) => Some(file.old_path().clone()),
                None => (self.parts.tree.contains(path) || self.rename_sources.contains(path))
                    .then(|| path.clone()),
            },
        }
    }

    /// Whether `path` is a file of the review: a changed file (by its new path) or a tree
    /// path. A rename's old path is not.
    #[must_use]
    pub fn has_file(&self, path: &RepoPath) -> bool {
        self.parts.tree.contains(path) || self.changed_by_path.contains_key(path)
    }
}

#[cfg(test)]
#[path = "review_snapshot_tests.rs"]
mod tests;
