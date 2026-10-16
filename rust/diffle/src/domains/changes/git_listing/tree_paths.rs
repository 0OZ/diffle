//! Every path on the new side of a comparison: what the file tree offers to open.

use std::collections::BTreeSet;

use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::git_error::GitError;

/// The paths the new side lists before any change is applied: a commit's own tree, or for the
/// worktree the index plus the untracked files.
pub async fn listed_new_side(
    repo: &GitRepo,
    new: &TreeSource,
) -> Result<BTreeSet<RepoPath>, GitError> {
    let listed = match new {
        TreeSource::Object(tree) => repo.tree_paths(tree).await?,
        TreeSource::Worktree => {
            let (index, untracked) = tokio::try_join!(repo.index_paths(), repo.untracked_paths())?;
            // A nested repository is listed as `dir/`; it has no file to open.
            index
                .into_iter()
                .chain(untracked.into_iter().filter(|path| !path.ends_with('/')))
                .collect()
        }
    };
    Ok(listed
        .iter()
        .map(|path| RepoPath::parse(path))
        .collect::<Result<BTreeSet<_>, _>>()?)
}

/// `listed` with `changed` applied on top: a deletion drops its path, a rename its source.
/// Listing and diffing are independent Git calls, so a caller runs them together and joins here.
#[must_use]
pub fn with_changes(mut paths: BTreeSet<RepoPath>, changed: &[ChangedFile]) -> BTreeSet<RepoPath> {
    for file in changed {
        match file.has_new_content() {
            false => paths.remove(&file.path),
            true => paths.insert(file.path.clone()),
        };
        if let Some(from) = file.kind.from_path() {
            paths.remove(from);
        }
    }
    paths
}

#[cfg(test)]
#[path = "tree_paths_tests.rs"]
mod tests;
