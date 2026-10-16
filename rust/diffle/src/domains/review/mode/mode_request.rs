//! Ways a reviewer changes what the session shows.

use crate::domains::revisions::entry_request::EntryRequest;

/// What to show instead of the current mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeRequest {
    /// A new review, resolved as at startup.
    Enter(EntryRequest),
    /// Narrow the current range to one of its parts, or back out of one.
    Focus(FocusTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusTarget {
    /// All of the range again.
    Range,
    /// The uncommitted changes of a range that ends at the worktree.
    Worktree,
    /// One of the range's listed commits, by hash or short hash.
    Commit(String),
}
