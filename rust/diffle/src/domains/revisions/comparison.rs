//! A comparison as the review keeps it: what was asked, whether it can move, and where its
//! comments are stored.

use super::comment_key::CommentKey;
use super::revspec::Revspec;

/// Whether the reviewed tree can change underneath the reviewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liveness {
    /// A side is the uncommitted tree: edits change the review.
    Worktree,
    /// Sides are named by refs: moving a ref changes the review.
    Refs,
    /// Both sides are fixed commits.
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    spec: Revspec,
    live: Liveness,
    comment_key: CommentKey,
}

impl Comparison {
    pub fn new(spec: Revspec, live: Liveness, comment_key: CommentKey) -> Self {
        Self {
            spec,
            live,
            comment_key,
        }
    }

    pub fn spec(&self) -> &Revspec {
        &self.spec
    }

    pub fn live(&self) -> Liveness {
        self.live
    }

    /// Fixed when the comparison is entered; discovery never changes comment storage.
    pub fn comment_key(&self) -> &CommentKey {
        &self.comment_key
    }
}
