//! One side of a comparison as the reviewer named it.

use std::fmt;

use super::revspec_error::RevspecError;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::names::tree_source::WORKTREE;

/// The keyword for the uncommitted tree, accepted wherever a revision is.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revision {
    /// The uncommitted tree: tracked edits, staged changes, and untracked files.
    Worktree,
    Rev(RevName),
}

impl Revision {
    pub fn head() -> Self {
        Self::Rev(RevName::head())
    }

    pub fn is_worktree(&self) -> bool {
        match self {
            Self::Worktree => true,
            Self::Rev(_) => false,
        }
    }

    pub fn parse(text: &str) -> Result<Self, RevspecError> {
        if text == WORKTREE {
            return Ok(Self::Worktree);
        }
        RevName::parse(text)
            .map(Self::Rev)
            .map_err(|source| RevspecError::Invalid {
                text: text.to_owned(),
                source,
            })
    }

    /// An empty side of `a..b` or `a...b` means `HEAD`, as it does for `git diff`.
    pub fn parse_or_head(text: &str) -> Result<Self, RevspecError> {
        match text.is_empty() {
            true => Ok(Self::head()),
            false => Self::parse(text),
        }
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Worktree => f.write_str(WORKTREE),
            Self::Rev(rev) => rev.fmt(f),
        }
    }
}
