//! Opens a review: an [`EntryRequest`] becomes the [`Comparison`] the session keeps.

use super::comment_keying::range_key;
use super::resolve_endpoints::{pin_commit, resolve_endpoints};
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::revisions::comment_key::{CommentKey, RangeShape};
use crate::domains::revisions::comparison::{Comparison, Liveness};
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revision::Revision;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::revisions::revspec_error::ResolveError;

/// A commit is pinned when entered: moving the ref it was named by, e.g. amending `HEAD`,
/// makes a different commit, and the review stays on this one.
pub async fn resolve_review(
    repo: &GitRepo,
    request: &EntryRequest,
) -> Result<Comparison, ResolveError> {
    match request {
        EntryRequest::Working => {
            let spec = Revspec::working();
            let resolved = resolve_endpoints(repo, &spec).await?;
            Ok(Comparison::new(spec, resolved.live, CommentKey::working()))
        }
        EntryRequest::Revspec(Revspec::Commit { rev }) => {
            let tip = pin_commit(repo, rev).await?;
            Ok(Comparison::new(
                Revspec::commit(RevName::of_object(&tip)),
                Liveness::Pinned,
                CommentKey::commit(&tip),
            ))
        }
        EntryRequest::Revspec(spec @ Revspec::Direct { old, new }) => {
            keyed_range(repo, spec, old, new, RangeShape::Direct).await
        }
        EntryRequest::Revspec(spec @ Revspec::MergeBase { old, new }) => {
            keyed_range(repo, spec, old, new, RangeShape::MergeBase).await
        }
    }
}

async fn keyed_range(
    repo: &GitRepo,
    spec: &Revspec,
    old: &Revision,
    new: &Revision,
    shape: RangeShape,
) -> Result<Comparison, ResolveError> {
    let resolved = resolve_endpoints(repo, spec).await?;
    let key = range_key(repo, old, new, shape, &resolved).await?;
    Ok(Comparison::new(spec.clone(), resolved.live, key))
}
