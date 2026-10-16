//! Turning a search request into what to grep: which files, and which lines of them.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::search::line_span::LineSpan;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::review::snapshot::snapshotter::Snapshotter;
use crate::kernel::bounded::map_limit;

/// Patches fetched at once to learn which lines a diff shows.
const PATCH_READS: usize = 8;

/// Where a search looks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    /// The one file named by `path`, if the review has it.
    File,
    /// The files the diff changed, except the deleted ones.
    #[default]
    Diff,
    /// Every file of the new side.
    Repo,
}

/// How much of those files is searched.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchContent {
    /// Only the lines the diff shows, context included.
    #[default]
    Diff,
    /// The whole files.
    Full,
}

/// The files to grep; `None` means the whole tree.
#[must_use]
pub fn files_in_scope(
    snapshot: &Snapshot,
    scope: SearchScope,
    path: Option<&RepoPath>,
) -> Option<Vec<RepoPath>> {
    match scope {
        SearchScope::Repo => None,
        SearchScope::File => Some(
            path.filter(|path| snapshot.parts().tree.contains(*path))
                .cloned()
                .into_iter()
                .collect(),
        ),
        SearchScope::Diff => Some(
            snapshot
                .parts()
                .changed
                .iter()
                .filter(|file| file.has_new_content())
                .map(|file| file.path.clone())
                .collect(),
        ),
    }
}

/// What the diff shows of each searchable changed file in `within` (all of them when `None`),
/// and the files that show anything.
pub async fn shown_lines(
    snapshotter: &Snapshotter,
    snapshot: &Snapshot,
    within: Option<&[RepoPath]>,
) -> Result<(Vec<RepoPath>, HashMap<RepoPath, Vec<LineSpan>>), ReviewError> {
    let candidates = searchable_files(&snapshot.parts().changed, within);
    let shown = map_limit(candidates, PATCH_READS, |path| async move {
        let spans = snapshotter.shown_spans(&path).await?;
        Ok::<_, ReviewError>((path, spans.map(|spans| spans.to_vec()).unwrap_or_default()))
    })
    .await;
    let mut ranges = HashMap::new();
    let mut files = Vec::new();
    for entry in shown {
        let (path, spans) = entry?;
        if !spans.is_empty() {
            files.push(path.clone());
        }
        ranges.insert(path, spans);
    }
    Ok((files, ranges))
}

/// The changed text files that still exist, limited to `within` when given.
#[must_use]
pub fn searchable_files(changed: &[ChangedFile], within: Option<&[RepoPath]>) -> Vec<RepoPath> {
    // A set: the list can name every changed file of a very large range.
    let wanted: Option<HashSet<&RepoPath>> = within.map(|paths| paths.iter().collect());
    changed
        .iter()
        .filter(|file| file.has_new_content() && !file.is_binary())
        .filter(|file| {
            wanted
                .as_ref()
                .is_none_or(|wanted| wanted.contains(&file.path))
        })
        .map(|file| file.path.clone())
        .collect()
}

#[cfg(test)]
#[path = "search_plan_tests.rs"]
mod tests;
