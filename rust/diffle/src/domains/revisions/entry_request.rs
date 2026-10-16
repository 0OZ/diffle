//! What the reviewer asked to open, before it is resolved.

use super::revspec::Revspec;

/// A request that enters a comparison from scratch, as the command line does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryRequest {
    /// The uncommitted changes, reviewed under the stable `working` key.
    Working,
    Revspec(Revspec),
}
