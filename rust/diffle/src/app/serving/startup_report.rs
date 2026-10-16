//! The status lines printed once the first review is open.

use crate::domains::changes::changed_file::{ChangedFile, Delta};
use crate::domains::revisions::comparison::Liveness;

const FILE_ONE: &str = "changed file";
const FILE_MANY: &str = "changed files";

/// Whether the review that just opened is watched for changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watching<'a> {
    On,
    /// Switched off with `--no-watch`.
    Disabled,
    /// Asked for, but the watcher could not start; says why.
    Failed(&'a str),
}

/// What the reviewer is told about the review that just opened. `follows` is whether moved refs
/// recompute it; `watching` whether file-system watching runs.
#[must_use]
pub fn report_lines(
    label: &str,
    changed: &[ChangedFile],
    liveness: Liveness,
    follows: bool,
    watching: Watching<'_>,
) -> Vec<String> {
    let mut lines = vec![format!("🔍 comparing {label}")];
    lines.push(match changed.is_empty() {
        true => "! No differences. Open any file from the tree to comment on it.".to_owned(),
        false => totals(changed),
    });
    let watched = match (liveness, follows) {
        (Liveness::Pinned, _) => None,
        (Liveness::Worktree, _) => Some("watching the worktree"),
        (Liveness::Refs, true) => Some("watching refs"),
        (Liveness::Refs, false) => Some("watching refs; moved refs show a Reload button"),
    };
    lines.extend(watched.map(|what| match watching {
        Watching::On => format!("👀 {what}"),
        Watching::Disabled => format!("👀 {what} (disabled with --no-watch)"),
        Watching::Failed(reason) => {
            format!("⚠ {reason}; the review updates when you reload or switch mode")
        }
    }));
    lines
}

fn totals(changed: &[ChangedFile]) -> String {
    let (additions, deletions) =
        changed
            .iter()
            .fold((0u64, 0u64), |(added, deleted), file| match file.delta {
                Delta::Text {
                    additions,
                    deletions,
                } => (added + u64::from(additions), deleted + u64::from(deletions)),
                Delta::Binary => (added, deleted),
            });
    let noun = match changed.len() {
        1 => FILE_ONE,
        _ => FILE_MANY,
    };
    format!("📝 {} {noun}  +{additions} −{deletions}", changed.len())
}

#[cfg(test)]
#[path = "startup_report_tests.rs"]
mod tests;
