//! Where the two sides of a comparison landed once resolved.

use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::revisions::comparison::Liveness;

/// The sides of a [`Revspec`](crate::domains::revisions::revspec::Revspec) after resolution.
/// A merge base or a parent has already replaced the old side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEndpoints {
    pub old: TreeSource,
    pub new: TreeSource,
    pub live: Liveness,
}
