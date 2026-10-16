//! The unified diffs of changed files, shaped for the client's parser.

use std::collections::HashSet;

use super::changed_file::ChangedFile;
use crate::domains::repository::diffing::diff_sides::DiffSides;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::kernel::bounded::map_limit;

/// Concurrent single-file patches (untracked files each need their own call).
const PATCH_CONCURRENCY: usize = 8;

/// The patch for one file. `untracked` says whether the file is known to be outside the
/// index; `None` probes, which only matters for a file the worktree adds.
pub async fn patch_for(
    repo: &GitRepo,
    sides: &DiffSides,
    file: &ChangedFile,
    context: u32,
    untracked: Option<bool>,
) -> Result<String, GitError> {
    if *sides == DiffSides::Nothing {
        return Ok(String::new());
    }
    if file.kind.is_missing_on_one_side(sides.is_reversed()) && sides.touches_worktree() {
        let outside_index = match untracked {
            Some(known) => known,
            None => !repo.is_tracked(&file.path).await?,
        };
        if outside_index {
            return repo.untracked_patch_text(sides, context, &file.path).await;
        }
    }
    repo.patch_text(sides, context, &renamed_pair(file)).await
}

/// The patches of every changed file, tracked ones in one call and untracked ones appended.
pub async fn patch_all(
    repo: &GitRepo,
    sides: &DiffSides,
    files: &[ChangedFile],
    context: u32,
) -> Result<String, GitError> {
    if *sides == DiffSides::Nothing {
        return Ok(String::new());
    }
    let tracked = repo.patch_text(sides, context, &[]);
    let untracked = untracked_candidates(repo, sides, files);
    let (tracked, untracked) = tokio::try_join!(tracked, untracked)?;
    // Owned files: a closure over borrowed ones is higher-ranked in the borrow, which the
    // compiler cannot prove `Send` once a request handler awaits this.
    let untracked: Vec<ChangedFile> = untracked.into_iter().cloned().collect();
    let extras = map_limit(untracked, PATCH_CONCURRENCY, |file| async move {
        patch_for(repo, sides, &file, context, Some(true)).await
    })
    .await;
    join(tracked, extras)
}

/// The patches of `files` only: tracked ones in one call, additions from the worktree each on
/// their own (they are probed). The caller bounds the batch, so the paths ride the arguments.
pub async fn patch_many(
    repo: &GitRepo,
    sides: &DiffSides,
    files: &[&ChangedFile],
    context: u32,
) -> Result<String, GitError> {
    if *sides == DiffSides::Nothing {
        return Ok(String::new());
    }
    let (single, batched): (Vec<&ChangedFile>, Vec<&ChangedFile>) =
        files.iter().copied().partition(|file| {
            sides.touches_worktree() && file.kind.is_missing_on_one_side(sides.is_reversed())
        });
    let paths: Vec<&RepoPath> = batched.iter().flat_map(|file| renamed_pair(file)).collect();
    let tracked = async {
        match batched.is_empty() {
            true => Ok(String::new()),
            false => repo.patch_text(sides, context, &paths).await,
        }
    };
    let single: Vec<ChangedFile> = single.into_iter().cloned().collect();
    let extras = map_limit(single, PATCH_CONCURRENCY, |file| async move {
        patch_for(repo, sides, &file, context, None).await
    });
    let (tracked, extras) = tokio::join!(tracked, extras);
    join(tracked?, extras)
}

/// Both names of a rename, or the one name of any other file.
fn renamed_pair(file: &ChangedFile) -> Vec<&RepoPath> {
    match file.kind.from_path() {
        Some(from) => vec![from, &file.path],
        None => vec![&file.path],
    }
}

/// Text files the worktree adds that the index does not know: the ones `diff` cannot show.
async fn untracked_candidates<'a>(
    repo: &GitRepo,
    sides: &DiffSides,
    files: &'a [ChangedFile],
) -> Result<Vec<&'a ChangedFile>, GitError> {
    if !sides.touches_worktree() {
        return Ok(Vec::new());
    }
    let added: Vec<&ChangedFile> = files
        .iter()
        .filter(|file| file.kind.is_missing_on_one_side(sides.is_reversed()) && !file.is_binary())
        .collect();
    if added.is_empty() {
        return Ok(added);
    }
    // The whole index rather than the added paths as arguments: a large diff would exceed argv.
    let tracked: HashSet<String> = repo.index_paths().await?.into_iter().collect();
    Ok(added
        .into_iter()
        .filter(|file| !tracked.contains(file.path.as_str()))
        .collect())
}

fn join(tracked: String, extras: Vec<Result<String, GitError>>) -> Result<String, GitError> {
    let mut patch = tracked;
    for extra in extras {
        patch.push_str(&extra?);
    }
    Ok(patch)
}

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;
