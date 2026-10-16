//! Resolves a [`Revspec`] to Git endpoints, for entering a review and for refreshing it.
//!
//! With `main` at C and `HEAD` at E, sharing ancestor B:
//!
//! ```text
//! A--B--C  main
//!     \--D--E  HEAD
//!
//! spec              old   new       live
//! main..HEAD        C     E         refs
//! main...HEAD       B     E         refs
//! HEAD..worktree    E     worktree  worktree
//! worktree..HEAD    worktree  E     worktree
//! main...worktree   B     worktree  worktree
//! <C hash>..<E hash>  C   E         pinned
//! <E hash>^!        D     E         pinned
//! ```
//!
//! A merge base replaces only the old side, taking `HEAD` for the worktree; a parent replaces
//! it with the commit's first parent, or the empty tree for a root commit. In a repository
//! without commits, `HEAD` compared directly with the worktree is the empty tree.

use tokio::try_join;

use super::resolved_endpoints::ResolvedEndpoints;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::revisions::comparison::Liveness;
use crate::domains::revisions::revision::Revision;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::revisions::revspec_error::{ResolveError, RevspecError};

/// What an unknown `HEAD` means for one resolution.
#[derive(Debug, Clone, Copy)]
enum UnbornHead {
    /// Compared directly with the worktree, a repository without commits has an empty past.
    EmptyTree,
    Unknown,
}

pub async fn resolve_endpoints(
    repo: &GitRepo,
    spec: &Revspec,
) -> Result<ResolvedEndpoints, ResolveError> {
    match spec {
        Revspec::Direct { old, new } => direct(repo, old, new).await,
        Revspec::MergeBase { old, new } => merge_base(repo, old, new).await,
        Revspec::Commit { rev } => commit(repo, rev).await,
    }
}

/// The commit `rev` names, or the reviewer's mistake.
pub async fn pin_commit(repo: &GitRepo, rev: &RevName) -> Result<ObjectId, ResolveError> {
    match repo.resolve_commit(rev).await? {
        Some(id) => Ok(id),
        None => Err(RevspecError::UnknownRevision(rev.clone()).into()),
    }
}

async fn direct(
    repo: &GitRepo,
    old: &Revision,
    new: &Revision,
) -> Result<ResolvedEndpoints, ResolveError> {
    let touches_worktree = old.is_worktree() || new.is_worktree();
    let unborn = match touches_worktree {
        true => UnbornHead::EmptyTree,
        false => UnbornHead::Unknown,
    };
    let (old_end, new_end) = try_join!(
        resolve_side(repo, old, unborn),
        resolve_side(repo, new, unborn)
    )?;
    let live = liveness(
        touches_worktree,
        is_pinned(old, &old_end) && is_pinned(new, &new_end),
    );
    Ok(ResolvedEndpoints {
        old: old_end,
        new: new_end,
        live,
    })
}

async fn merge_base(
    repo: &GitRepo,
    old: &Revision,
    new: &Revision,
) -> Result<ResolvedEndpoints, ResolveError> {
    let (old_end, new_end) = try_join!(
        resolve_side(repo, old, UnbornHead::Unknown),
        resolve_side(repo, new, UnbornHead::Unknown)
    )?;
    let (a, b) = try_join!(
        commit_or_head(repo, &old_end),
        commit_or_head(repo, &new_end)
    )?;
    let Some(base) = repo.merge_base(&a, &b).await? else {
        return Err(RevspecError::NoMergeBase {
            old: old.clone(),
            new: new.clone(),
        }
        .into());
    };
    let live = liveness(
        old.is_worktree() || new.is_worktree(),
        is_pinned(old, &old_end) && is_pinned(new, &new_end),
    );
    Ok(ResolvedEndpoints {
        old: TreeSource::Object(base),
        new: new_end,
        live,
    })
}

async fn commit(repo: &GitRepo, rev: &RevName) -> Result<ResolvedEndpoints, ResolveError> {
    let tip = pin_commit(repo, rev).await?;
    let parent = match repo
        .resolve_commit(&RevName::of_object(&tip).parent())
        .await?
    {
        Some(parent) => parent,
        None => repo.empty_tree().await?,
    };
    let live = liveness(false, tip.is_abbreviated_by(rev.as_str()));
    Ok(ResolvedEndpoints {
        old: TreeSource::Object(parent),
        new: TreeSource::Object(tip),
        live,
    })
}

async fn resolve_side(
    repo: &GitRepo,
    revision: &Revision,
    unborn: UnbornHead,
) -> Result<TreeSource, ResolveError> {
    let Revision::Rev(name) = revision else {
        return Ok(TreeSource::Worktree);
    };
    match (repo.resolve_commit(name).await?, unborn) {
        (Some(id), _) => Ok(TreeSource::Object(id)),
        (None, UnbornHead::EmptyTree) if name.is_head() => {
            Ok(TreeSource::Object(repo.empty_tree().await?))
        }
        (None, UnbornHead::EmptyTree | UnbornHead::Unknown) => {
            Err(RevspecError::UnknownRevision(name.clone()).into())
        }
    }
}

/// The commit a merge base is taken from: the worktree sits on `HEAD`.
async fn commit_or_head(repo: &GitRepo, endpoint: &TreeSource) -> Result<ObjectId, ResolveError> {
    match endpoint {
        TreeSource::Object(id) => Ok(id.clone()),
        TreeSource::Worktree => pin_commit(repo, &RevName::head()).await,
    }
}

fn liveness(touches_worktree: bool, pinned: bool) -> Liveness {
    match (touches_worktree, pinned) {
        (true, _) => Liveness::Worktree,
        (false, true) => Liveness::Pinned,
        (false, false) => Liveness::Refs,
    }
}

/// Whether the reviewer named the side by its hash, so no ref can move it.
fn is_pinned(revision: &Revision, endpoint: &TreeSource) -> bool {
    match (revision, endpoint) {
        (Revision::Rev(name), TreeSource::Object(id)) => id.is_abbreviated_by(name.as_str()),
        (Revision::Rev(_), TreeSource::Worktree) | (Revision::Worktree, _) => false,
    }
}
