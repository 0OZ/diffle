//! What the session is showing: a comparison, possibly one commit (or the uncommitted
//! changes) focused within the range it belongs to.

use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::revisions::comment_key::CommentKey;
use crate::domains::revisions::comparison::{Comparison, Liveness};
use crate::domains::revisions::revspec::Revspec;

/// A comparison with an optional enclosing range. While a commit is focused the range keeps
/// listing its commits and is where "all changes" leads back to; the focused comparison keeps
/// its own comments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewMode {
    comparison: Comparison,
    within: Option<Comparison>,
}

impl ReviewMode {
    #[must_use]
    pub const fn whole(comparison: Comparison) -> Self {
        Self {
            comparison,
            within: None,
        }
    }

    /// `commit` of `range`, against its first parent. The commit is pinned; the refs the range
    /// was named by keep moving, so the focus lives exactly as long as the range does.
    #[must_use]
    pub fn commit_within(commit: &ObjectId, range: Comparison) -> Self {
        let live = match range.live() {
            Liveness::Pinned => Liveness::Pinned,
            Liveness::Worktree | Liveness::Refs => Liveness::Refs,
        };
        let comparison = Comparison::new(
            Revspec::commit(
                crate::domains::repository::names::rev_name::RevName::of_object(commit),
            ),
            live,
            CommentKey::commit(commit),
        );
        Self {
            comparison,
            within: Some(range),
        }
    }

    /// The uncommitted changes of `range`, which ends at the worktree. They share `working`'s comments.
    #[must_use]
    pub fn worktree_within(range: Comparison) -> Self {
        Self {
            comparison: Comparison::new(
                Revspec::working(),
                Liveness::Worktree,
                CommentKey::working(),
            ),
            within: Some(range),
        }
    }

    /// What is on screen.
    #[must_use]
    pub const fn comparison(&self) -> &Comparison {
        &self.comparison
    }

    /// The enclosing range while something is focused within it.
    #[must_use]
    pub const fn within(&self) -> Option<&Comparison> {
        self.within.as_ref()
    }

    /// The range whose commits are listed and whose refs are followed: the enclosing one while
    /// focused, else the comparison itself.
    #[must_use]
    pub fn range(&self) -> &Comparison {
        self.within.as_ref().unwrap_or(&self.comparison)
    }

    /// Back to the enclosing range, or `self` when nothing is focused.
    #[must_use]
    pub fn unfocused(&self) -> Self {
        Self::whole(self.range().clone())
    }

    #[must_use]
    pub fn is_single_commit(&self) -> bool {
        matches!(self.comparison.spec(), Revspec::Commit { .. })
    }
}

#[cfg(test)]
#[path = "review_mode_tests.rs"]
mod tests;
