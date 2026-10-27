//! The review's logical endpoints and identity, independent of our owned physical checkout.
use super::{Active, Session};
use crate::error::Result;
use crate::git::{GitRepo, WORKTREE};
use crate::mode::keys::range_comment_key;
use crate::mode::resolve::{names_branch, resolve_range};
use crate::protocol::{Live, Review};

pub(super) struct Basis {
    pub(super) resolution: Review,
    pub(super) range_key: Option<String>,
    pub(super) followed: bool,
    pub(super) branches: bool,
}

impl Session {
    pub(super) async fn review_basis(&self, review: &Review, previous: Option<&Active>) -> Result<Basis> {
        if self.repo.is_isolated()
            && let Some(previous) = previous
        {
            let mut resolution = review.clone();
            if let (Review::Range { old, new, .. }, Review::Range { old: before_old, new: before_new, .. }) =
                (&mut resolution, &previous.resolution)
            {
                old.clone_from(before_old);
                new.clone_from(before_new);
            }
            return Ok(Basis {
                resolution,
                range_key: previous.range_key.clone(),
                followed: previous.followed,
                branches: previous.branches,
            });
        }
        let (range_key, followed, branches) = match review {
            Review::Commit { .. } => (None, false, false),
            Review::Range { old, new, base, pr, .. } => {
                let ends = resolve_range(&self.repo, old, new, *base).await?;
                let key = range_comment_key(
                    &self.repo,
                    &self.github.instance,
                    (old, new, *base),
                    pr.as_ref(),
                    (&ends.old_sha, &ends.new_sha),
                )
                .await?;
                let followed = ends.live == Live::Refs || pr.is_some();
                let branches = ends.live == Live::Refs && names_branch(&self.repo, old, new).await?;
                (Some(key), followed, branches)
            }
        };
        let mut resolution = review.clone();
        if self.repo.is_isolated()
            && let Review::Range { old, new, .. } = &mut resolution
            && old != WORKTREE
            && new != WORKTREE
        {
            pin_head(&self.repo, old).await?;
            pin_head(&self.repo, new).await?;
        }
        Ok(Basis { resolution, range_key, followed, branches })
    }
}

async fn pin_head(repo: &GitRepo, rev: &mut String) -> Result<()> {
    let suffix = rev.strip_prefix("HEAD").or_else(|| rev.strip_prefix('@'));
    if suffix.is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(['^', '~']) || suffix.starts_with("@{")) {
        *rev = repo.resolve(rev).await?;
    }
    Ok(())
}
