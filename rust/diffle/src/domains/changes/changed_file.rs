//! One file that differs between the two sides.

use super::change_kind::ChangeKind;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;

/// How many lines a change touches, or that the file has none to count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delta {
    Text { additions: u32, deletions: u32 },
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// The new path; for a deletion, the path that was deleted.
    pub path: RepoPath,
    pub kind: ChangeKind,
    pub delta: Delta,
    /// The new side's blob. A worktree side's is `None` when the file could not be hashed.
    pub blob: Option<ObjectId>,
    /// The old side's blob.
    pub old_blob: Option<ObjectId>,
    /// A gitattribute, a path convention or a content sniff says a generator wrote it.
    pub generated: bool,
    /// A gitlink (mode 160000): `blob` is the recorded commit and there is no file to open.
    pub submodule: bool,
}

impl ChangedFile {
    #[must_use]
    pub const fn is_binary(&self) -> bool {
        matches!(self.delta, Delta::Binary)
    }

    /// Where the old side keeps the file: the rename's source, else the same path.
    #[must_use]
    pub fn old_path(&self) -> &RepoPath {
        self.kind.from_path().unwrap_or(&self.path)
    }

    /// Whether anything can be read from the new side.
    #[must_use]
    pub const fn has_new_content(&self) -> bool {
        !self.kind.is_deleted()
    }

    /// Whether a text sniff makes sense: there is a new-side file, and it is not binary.
    #[must_use]
    pub const fn is_textual(&self) -> bool {
        self.has_new_content() && !self.is_binary() && !self.submodule
    }
}
