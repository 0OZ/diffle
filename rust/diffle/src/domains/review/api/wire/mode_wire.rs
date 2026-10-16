//! `ModeSpecSchema`: the comparison on screen, and the range it sits in while a commit is
//! focused.

use serde::Serialize;

use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::revisions::comparison::{Comparison, Liveness};
use crate::domains::revisions::revspec::Revspec;

const DIRECT: &str = "direct";
const MERGE_BASE: &str = "merge-base";
const PARENT: &str = "parent";

/// `ComparisonSchema`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonWire {
    old: String,
    new: String,
    base: &'static str,
    live: &'static str,
    comment_key: String,
}

#[derive(Debug, Serialize)]
pub struct ModeSpecWire {
    #[serde(flatten)]
    comparison: ComparisonWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    within: Option<ComparisonWire>,
}

impl From<&Comparison> for ComparisonWire {
    fn from(comparison: &Comparison) -> Self {
        let (old, new, base) = match comparison.spec() {
            Revspec::Direct { old, new } => (old.to_string(), new.to_string(), DIRECT),
            Revspec::MergeBase { old, new } => (old.to_string(), new.to_string(), MERGE_BASE),
            // Under `parent` both ends name the one commit.
            Revspec::Commit { rev } => (rev.to_string(), rev.to_string(), PARENT),
        };
        let live = match comparison.live() {
            Liveness::Worktree => "worktree",
            Liveness::Refs => "refs",
            Liveness::Pinned => "none",
        };
        Self {
            old,
            new,
            base,
            live,
            comment_key: comparison.comment_key().to_string(),
        }
    }
}

impl From<&ReviewMode> for ModeSpecWire {
    fn from(mode: &ReviewMode) -> Self {
        Self {
            comparison: mode.comparison().into(),
            within: mode.within().map(ComparisonWire::from),
        }
    }
}
