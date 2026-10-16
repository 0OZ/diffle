//! A comparison as the reviewer reads it.

use crate::domains::revisions::comparison::Comparison;
use crate::domains::revisions::revision::Revision;
use crate::domains::revisions::revspec::Revspec;

const ABBREVIATED: usize = 7;
const LOCAL_BRANCHES: &str = "refs/heads/";
const REMOTE_BRANCHES: &str = "refs/remotes/";

/// `old..new`, `old...new`, or `<short>^!`, with branch names instead of ref namespaces.
#[must_use]
pub fn comparison_label(comparison: &Comparison) -> String {
    match comparison.spec() {
        Revspec::Direct { old, new } => format!("{}..{}", side(old), side(new)),
        Revspec::MergeBase { old, new } => format!("{}...{}", side(old), side(new)),
        Revspec::Commit { rev } => {
            let name = rev.as_str();
            format!("{}^!", name.get(..ABBREVIATED).unwrap_or(name))
        }
    }
}

fn side(revision: &Revision) -> String {
    match revision {
        Revision::Worktree => revision.to_string(),
        Revision::Rev(name) => ref_name(name.as_str()).to_owned(),
    }
}

/// A revision as the reviewer typed it: without `refs/heads/` or `refs/remotes/`.
fn ref_name(rev: &str) -> &str {
    rev.strip_prefix(LOCAL_BRANCHES)
        .or_else(|| rev.strip_prefix(REMOTE_BRANCHES))
        .unwrap_or(rev)
}

#[cfg(test)]
#[path = "comparison_label_tests.rs"]
mod tests;
