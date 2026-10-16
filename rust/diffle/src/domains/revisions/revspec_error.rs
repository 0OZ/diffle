//! Why a comparison could not be formed: the reviewer's input, or Git itself.

use thiserror::Error;

use super::revision::Revision;
use crate::domains::repository::names::rev_name::{RevName, RevNameError};
use crate::domains::repository::process::git_error::GitError;

/// The reviewer's arguments do not describe a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RevspecError {
    #[error("expected one or two revisions")]
    ArgCount,
    #[error("cannot combine \"..\" with two revisions")]
    RangeWithTwoRevisions,
    #[error("cannot combine \"^!\" with two revisions")]
    CommitWithTwoRevisions,
    #[error("cannot combine \"^!\" with \"..\"")]
    CommitWithRange,
    #[error("the worktree is not a commit")]
    WorktreeIsNotACommit,
    #[error("invalid revision {text:?}: {source}")]
    Invalid {
        text: String,
        #[source]
        source: RevNameError,
    },
    #[error("unknown revision: {0}")]
    UnknownRevision(RevName),
    #[error("no merge base between {old} and {new}")]
    NoMergeBase { old: Revision, new: Revision },
}

/// Resolving a comparison against a repository failed.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error(transparent)]
    Revspec(#[from] RevspecError),
    #[error(transparent)]
    Git(#[from] GitError),
}
