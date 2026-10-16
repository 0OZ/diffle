//! The files that differ between two sides, with line counts and each side's blob.

use std::collections::HashSet;

use super::change_kind::ChangeKind;
use super::changed_file::ChangedFile;
use super::git_listing::raw_parse::parse_raw_numstat;
use super::git_listing::untracked_size::untracked_delta;
use crate::domains::repository::contents::worktree_hashes::WorktreeEntry;
use crate::domains::repository::diffing::diff_sides::DiffSides;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::git_error::GitError;
use crate::kernel::bounded::map_limit;

/// Concurrent worktree reads while sizing untracked files.
const UNTRACKED_CONCURRENCY: usize = 8;

/// Changed files between `old` and `new`, sorted by path. When the worktree is a side, its
/// untracked files are included and its blobs are hashed from disk, since Git has no object
/// for them.
pub async fn changed_files(
    repo: &GitRepo,
    old: &TreeSource,
    new: &TreeSource,
) -> Result<Vec<ChangedFile>, GitError> {
    let sides = DiffSides::new(old, new);
    if sides == DiffSides::Nothing {
        return Ok(Vec::new());
    }
    let reversed = sides.is_reversed();
    let (records, untracked) = tokio::try_join!(repo.raw_numstat(&sides), async {
        match sides.touches_worktree() {
            true => repo.untracked_paths().await,
            false => Ok(Vec::new()),
        }
    })?;
    let mut files = parse_raw_numstat(&records)?;
    let known: HashSet<&str> = files
        .iter()
        .flat_map(|file| {
            [
                Some(file.path.as_str()),
                file.kind.from_path().map(RepoPath::as_str),
            ]
        })
        .flatten()
        .collect();
    // A nested repository is listed as `dir/`; Git has no diff for it.
    let fresh: Vec<String> = untracked
        .iter()
        .filter(|path| !known.contains(path.as_str()) && !path.ends_with('/'))
        .cloned()
        .collect();
    let extras = map_limit(fresh, UNTRACKED_CONCURRENCY, |path| {
        untracked_file(repo, path, reversed)
    })
    .await;
    for extra in extras {
        files.push(extra?);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if new.is_worktree() {
        hash_sides(repo, &mut files, Side::New).await?;
    }
    if reversed {
        hash_sides(repo, &mut files, Side::Old).await?;
    }
    Ok(files)
}

async fn untracked_file(
    repo: &GitRepo,
    path: String,
    reversed: bool,
) -> Result<ChangedFile, GitError> {
    let path = RepoPath::parse(&path)?;
    let delta = untracked_delta(&repo.root().join(path.as_str()), reversed).await;
    let kind = match reversed {
        true => ChangeKind::Deleted,
        false => ChangeKind::Added,
    };
    Ok(ChangedFile {
        path,
        kind,
        delta,
        blob: None,
        old_blob: None,
        generated: false,
        submodule: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Old,
    New,
}

/// Fills the blob of the side that is the worktree, for the files that exist there.
async fn hash_sides(repo: &GitRepo, files: &mut [ChangedFile], side: Side) -> Result<(), GitError> {
    let present: Vec<&mut ChangedFile> = files
        .iter_mut()
        .filter(|file| match side {
            Side::New => file.has_new_content(),
            Side::Old => !file.kind.is_added(),
        })
        .collect();
    let entries: Vec<WorktreeEntry> = present
        .iter()
        .map(|file| {
            let path = match side {
                Side::New => file.path.clone(),
                Side::Old => file.old_path().clone(),
            };
            match file.submodule {
                true => WorktreeEntry::Submodule(path),
                false => WorktreeEntry::File(path),
            }
        })
        .collect();
    let hashes = repo.hash_worktree(&entries).await?;
    for (file, hash) in present.into_iter().zip(hashes) {
        match side {
            Side::New => file.blob = hash,
            Side::Old => file.old_blob = hash,
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "changed_files_tests.rs"]
mod tests;
