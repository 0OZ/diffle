//! Derives the key a range's comments are stored under (see [`CommentKey`]).

use super::resolved_endpoints::ResolvedEndpoints;
use crate::domains::repository::git_refs::Remote;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::hosted_repo::HostedRepo;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::revisions::comment_key::{CommentKey, KeySide, RangeShape};
use crate::domains::revisions::revision::Revision;
use tokio::try_join;

/// The key of `old` against `new`. Each side is keyed by its branch identity when it names a
/// branch, else by what it resolved to (`resolved` has the merge base already applied).
pub async fn range_key(
    repo: &GitRepo,
    old: &Revision,
    new: &Revision,
    shape: RangeShape,
    resolved: &ResolvedEndpoints,
) -> Result<CommentKey, GitError> {
    let remotes = repo.remotes().await?;
    let (old_identity, new_identity) = try_join!(
        branch_identity(repo, &remotes, old),
        branch_identity(repo, &remotes, new)
    )?;
    Ok(CommentKey::range(
        &old_identity.unwrap_or_else(|| side_of(&resolved.old)),
        &new_identity.unwrap_or_else(|| side_of(&resolved.new)),
        shape,
    ))
}

fn side_of(endpoint: &TreeSource) -> KeySide {
    match endpoint {
        TreeSource::Worktree => KeySide::Worktree,
        TreeSource::Object(id) => KeySide::Object(id.clone()),
    }
}

/// `owner/repo:branch` (lowercase) for a branch tracking a GitHub remote, which keeps forks
/// apart; the full ref for any other branch; `None` for a hash, an expression, or the worktree.
async fn branch_identity(
    repo: &GitRepo,
    remotes: &[Remote],
    revision: &Revision,
) -> Result<Option<KeySide>, GitError> {
    let Revision::Rev(name) = revision else {
        return Ok(None);
    };
    let Some(branch) = repo.branch_ref(name).await? else {
        return Ok(None);
    };
    let hosted = match repo.upstream_branch(&branch, remotes).await? {
        Some(upstream) => remotes
            .iter()
            .find(|remote| remote.name == upstream.remote)
            .and_then(|remote| HostedRepo::parse_github(&remote.url))
            .map(|hosted| format!("{}:{}", hosted.slug(), upstream.branch)),
        None => None,
    };
    Ok(Some(KeySide::Branch(
        hosted.unwrap_or_else(|| branch.full_name()),
    )))
}
