//! What the session tells the rest of the program as the review changes.

use crate::domains::review::snapshot::review_snapshot::Moved;
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// A new snapshot is current; clients refetch.
    Snapshot(SnapshotVersion),
    /// The compared refs left the shown snapshot behind, or came back to it (`None`).
    Moved(Option<Moved>),
    /// Threads were added, edited, resolved or removed; clients refetch them.
    Threads,
    /// Viewed marks changed.
    Viewed,
    /// The user config changed.
    Config,
}
