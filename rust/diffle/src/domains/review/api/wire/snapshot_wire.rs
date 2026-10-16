//! `SnapshotSchema`: one consistent view of the review.

use std::borrow::Cow;

use serde::Serialize;

use super::changed_file_wire::ChangedFileWire;
use super::commits_wire::{RangeCommitsWire, ReviewedCommitWire};
use super::mode_wire::ModeSpecWire;
use crate::domains::review::snapshot::review_snapshot::Snapshot;

/// The recorded iterations of a range. This server records none, so the list is always empty;
/// the type keeps the field from carrying anything else.
#[derive(Debug, Serialize)]
struct NoIterations([(); 0]);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotWire<'a> {
    root: Cow<'a, str>,
    mode: ModeSpecWire,
    version: u64,
    /// Resolved endpoints; either can be the `worktree` sentinel.
    old_sha: String,
    new_sha: String,
    /// The checked-out commit; empty on an unborn branch.
    head_sha: &'a str,
    context: u32,
    changed: Vec<ChangedFileWire<'a>>,
    /// Every path on the new side, sorted.
    tree: Vec<&'a str>,
    commit: Option<ReviewedCommitWire<'a>>,
    commits: RangeCommitsWire<'a>,
    iterations: NoIterations,
}

impl<'a> From<&'a Snapshot> for SnapshotWire<'a> {
    fn from(snapshot: &'a Snapshot) -> Self {
        let parts = snapshot.parts();
        Self {
            root: parts.root.to_string_lossy(),
            mode: ModeSpecWire::from(&parts.mode),
            version: parts.version.get(),
            old_sha: parts.old.to_string(),
            new_sha: parts.new.to_string(),
            head_sha: parts.head.as_ref().map_or("", |head| head.as_str()),
            context: parts.context.get(),
            changed: parts.changed.iter().map(Into::into).collect(),
            tree: parts.tree.iter().map(|path| path.as_str()).collect(),
            commit: parts.commit.as_ref().map(Into::into),
            commits: RangeCommitsWire::from(&parts.commits),
            iterations: NoIterations([]),
        }
    }
}
