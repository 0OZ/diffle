//! Derives a [`Snapshot`] from a mode: resolve both sides, then list what changed, what the
//! new side holds and which commits the comparison spans, all at once.

use crate::domains::changes::changed_files::changed_files;
use crate::domains::changes::generated_files::generated_detector::GeneratedDetector;
use crate::domains::changes::git_listing::tree_paths::{listed_new_side, with_changes};
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::history::range_commits::{CommitRange, RangeCommits};
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::snapshot::review_snapshot::{
    ReviewedCommit, Snapshot, SnapshotParts, SpannedCommits,
};
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;
use crate::domains::revisions::resolution::resolve_endpoints::resolve_endpoints;
use crate::domains::settings::context_lines::ContextLines;

/// The most commits a snapshot lists; `total` still counts them all.
pub const MAX_RANGE_COMMITS: usize = 250;

pub async fn compute_snapshot(
    repo: &GitRepo,
    detector: &GeneratedDetector,
    mode: &ReviewMode,
    version: SnapshotVersion,
    context: ContextLines,
) -> Result<Snapshot, ReviewError> {
    let (endpoints, head) = tokio::try_join!(
        async {
            resolve_endpoints(repo, mode.comparison().spec())
                .await
                .map_err(ReviewError::from)
        },
        async {
            repo.resolve_commit(&RevName::head())
                .await
                .map_err(ReviewError::from)
        },
    )?;
    let (old, new) = (endpoints.old, endpoints.new);
    // The tree belongs to the selected new side: a commit's own listing, or the index plus
    // untracked files for the worktree. Listing and diffing are independent Git calls.
    let (listed, mut changed, commit, commits) = tokio::try_join!(
        async { listed_new_side(repo, &new).await.map_err(ReviewError::from) },
        async {
            changed_files(repo, &old, &new)
                .await
                .map_err(ReviewError::from)
        },
        reviewed_commit(repo, mode, &new),
        spanned_commits(repo, mode, (&old, &new), head.as_ref()),
    )?;
    detector.flag(repo, &mut changed, &new).await;
    let tree = with_changes(listed, &changed);
    Ok(Snapshot::new(SnapshotParts {
        root: repo.root().to_path_buf(),
        mode: mode.clone(),
        version,
        old,
        new,
        head,
        context,
        changed,
        tree,
        commit,
        commits,
    }))
}

/// The commit under review, when the comparison is a single commit against its parent.
async fn reviewed_commit(
    repo: &GitRepo,
    mode: &ReviewMode,
    new: &TreeSource,
) -> Result<Option<ReviewedCommit>, ReviewError> {
    let (true, TreeSource::Object(id)) = (mode.is_single_commit(), new) else {
        return Ok(None);
    };
    let rev = RevName::of_object(id);
    let (info, neighbours) = tokio::try_join!(repo.commit_info(&rev), repo.commit_neighbours(id))?;
    Ok(info.map(|info| ReviewedCommit { info, neighbours }))
}

/// The commits the shown range spans. While a commit is focused that is its range's, between
/// the range's own endpoints.
async fn spanned_commits(
    repo: &GitRepo,
    mode: &ReviewMode,
    shown: (&TreeSource, &TreeSource),
    head: Option<&ObjectId>,
) -> Result<SpannedCommits, ReviewError> {
    if let Some(range) = mode.within() {
        let endpoints = resolve_endpoints(repo, range.spec()).await?;
        return spanned(repo, endpoints.old, endpoints.new, head).await;
    }
    let (old, new) = (shown.0.clone(), shown.1.clone());
    match (mode.is_single_commit(), &new) {
        (true, TreeSource::Object(tip)) => {
            let commits = repo
                .range_commits(&CommitRange::Single(tip.clone()), MAX_RANGE_COMMITS)
                .await?;
            Ok(SpannedCommits { commits, old, new })
        }
        (true, TreeSource::Worktree) | (false, _) => spanned(repo, old, new, head).await,
    }
}

/// `old..new` with the worktree standing for `HEAD`. A side that is not a commit (an unborn
/// branch has no `HEAD`) or two sides at one commit span nothing.
async fn spanned(
    repo: &GitRepo,
    old: TreeSource,
    new: TreeSource,
    head: Option<&ObjectId>,
) -> Result<SpannedCommits, ReviewError> {
    let commit_of = |side: &TreeSource| match side {
        TreeSource::Object(id) => Some(id.clone()),
        TreeSource::Worktree => head.cloned(),
    };
    let commits = match (commit_of(&old), commit_of(&new)) {
        (Some(from), Some(to)) if from != to => {
            repo.range_commits(&CommitRange::Span { from, to }, MAX_RANGE_COMMITS)
                .await?
        }
        (Some(_) | None, Some(_) | None) => RangeCommits::default(),
    };
    Ok(SpannedCommits { commits, old, new })
}

#[cfg(test)]
#[path = "snapshot_compute_tests.rs"]
mod tests;
