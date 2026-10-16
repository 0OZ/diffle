//! Why the session could not do what was asked.

use thiserror::Error;

use crate::domains::comments::storage::store_error::CommentError;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::revisions::revspec_error::{ResolveError, RevspecError};

#[derive(Debug, Error)]
pub enum ReviewError {
    #[error(transparent)]
    Revspec(#[from] RevspecError),
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Comments(#[from] CommentError),
    #[error("a single commit has no commits to focus")]
    NothingToFocus,
    #[error("{range} has no uncommitted changes")]
    NoUncommittedChanges { range: String },
    #[error("not a listed commit of {range}: {commit}")]
    NotListed { range: String, commit: String },
    #[error("no review is open yet")]
    NotOpen,
    #[error("the review stopped before the request finished")]
    Interrupted,
}
