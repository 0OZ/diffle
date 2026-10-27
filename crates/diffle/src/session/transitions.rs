//! Entering a review and showing a slice of the active range: resolving what was asked, then
//! activating it in place of the current review.
use std::sync::{Arc, Mutex};

use super::logical::Basis;
use super::{Active, Session};
use crate::comments::CommentStore;
use crate::error::{Error, Result};
use crate::iterations::IterationStore;
use crate::mode::resolve::resolve_review;
use crate::mode::view::resolve_shown;
use crate::protocol::{
    EnterRequest, RangeRef, RangeView, Review, ServerMessage, Snapshot, comparison_label, range_of, with_view,
};
use crate::snapshotter::Snapshotter;

impl Session {
    /// Resolves a request into a review without activating it, so a caller can reject invalid input
    /// before serving. Queued, so `close` waits for it.
    pub fn resolve(self: &Arc<Self>, req: EnterRequest) -> impl Future<Output = Result<Review>> + Send + use<> {
        let this = self.clone();
        self.run(async move { resolve_review(&req, &this.repo, &this.github).await })
    }

    /// First review, from `resolve`. Fails `ready` too, so early requests error out.
    pub async fn start(self: &Arc<Self>, review: Review) -> Result<Arc<Snapshot>> {
        let this = self.clone();
        let result = self.run(async move { this.activate(review).await }).await;
        self.ready.send_replace(Some(result.as_ref().map(|_| ()).map_err(Clone::clone)));
        result
    }

    /// Enters another review at runtime. Broadcasts a snapshot bump on success.
    pub async fn enter(self: &Arc<Self>, req: EnterRequest) -> Result<Arc<Snapshot>> {
        let this = self.clone();
        let snap = self
            .run(async move {
                let review = resolve_review(&req, &this.repo, &this.github).await?;
                this.activate(review).await
            })
            .await?;
        self.hub.broadcast(ServerMessage::Snapshot { version: snap.version });
        Ok(snap)
    }

    /// Shows a slice of the active range: one of its commits, its uncommitted changes, an
    /// interdiff of two of its iterations or one pair, or the whole range again.
    pub async fn show(self: &Arc<Self>, view: RangeView) -> Result<Arc<Snapshot>> {
        let this = self.clone();
        let snap = self
            .run(async move {
                let review = this.view(view).await?;
                this.activate_context(review, this.context()).await
            })
            .await?;
        self.hub.broadcast(ServerMessage::Snapshot { version: snap.version });
        Ok(snap)
    }

    pub(super) async fn activate(self: &Arc<Self>, review: Review) -> Result<Arc<Snapshot>> {
        self.activate_with(review, self.context(), None).await
    }

    pub(super) async fn activate_context(self: &Arc<Self>, review: Review, context: u32) -> Result<Arc<Snapshot>> {
        self.activate_with(review, context, Some(self.active()?)).await
    }

    async fn activate_with(
        self: &Arc<Self>,
        review: Review,
        context: u32,
        previous: Option<Arc<Active>>,
    ) -> Result<Arc<Snapshot>> {
        let _guard = self.workspace.gate.write().await;
        self.workspace.check()?;
        let version = self.next_version();
        let Basis { resolution, range_key, followed, branches } =
            self.review_basis(&review, previous.as_deref()).await?;
        let iterations = followed.then(|| IterationStore::new(self.repo.clone(), range_key.clone().expect("a range")));
        // Resolve from the logical range, not HEAD after one of our own focused checkouts.
        let shown = resolve_shown(&self.repo, &resolution, iterations.as_ref(), range_key.as_deref()).await?;
        let snapshotter =
            Snapshotter::new(self.repo.clone(), review.clone(), range_key.clone(), version, context, iterations);
        if self.repo.is_isolated() && shown.old_sha == crate::git::WORKTREE && shown.new_sha != crate::git::WORKTREE {
            return Err(Error::revspec("cannot change the isolated checkout while its worktree is the old side"));
        }
        let previous_head = self.checkout_previous(&shown.new_sha).await?;
        let prev = self.active.lock().unwrap().clone();
        if self.repo.is_isolated()
            && let Some(prev) = &prev
            && let Some(watcher) = prev.watcher.lock().await.take()
        {
            // Re-baseline metadata after our own checkout/iteration refs, not on its dirty signals.
            watcher.close().await;
        }
        let prepared = async {
            if previous_head.is_some() {
                self.reset_workspace_services().await;
                self.repo.checkout_owned(&shown.new_sha).await?;
            }
            let snap = snapshotter.current_resolved(shown).await?;
            let comments = Arc::new(CommentStore::open(self.repo.review_state_dir(), &snap.comment_key).await?);
            let next = Arc::new(Active {
                review,
                resolution,
                followed,
                range_key,
                follows: snap.follows,
                snapshotter,
                comments,
                watcher: tokio::sync::Mutex::new(None),
                moved: Mutex::new(None),
                branches,
            });
            self.relocate_comments(&next, &snap).await?;
            self.watch_active(&next).await?;
            Ok::<_, Error>((next, snap))
        }
        .await;
        let (next, snap) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(head) = &previous_head {
                    self.restore_checkout(head, &error).await?;
                }
                if self.repo.is_isolated()
                    && let Some(prev) = &prev
                {
                    if previous_head.is_some() {
                        self.resume_workspace_services(prev.snapshotter.current().await?).await;
                    }
                    self.watch_active(prev).await?;
                }
                return Err(error);
            }
        };
        self.active.lock().unwrap().replace(next);
        if let Some(prev) = prev
            && let Some(watcher) = prev.watcher.lock().await.take()
        {
            watcher.close().await;
        }
        *self.context.lock().unwrap() = context;
        self.announce(&snap);
        Ok(snap)
    }

    /// The active range with `view`, abbreviated hashes expanded against the current snapshot: a
    /// commit must be one of the range's listed commits, a pair one of the shown interdiff's. The
    /// rest (a missing iteration, a range without uncommitted changes) fails when the slice resolves.
    pub(super) async fn view(self: &Arc<Self>, view: RangeView) -> Result<Review> {
        let a = self.active()?;
        let Some(range) = range_of(&a.review) else {
            return Err(Error::revspec(match view {
                RangeView::Interdiff { .. } => "a single commit has no iterations",
                _ => "a single commit has no commits to focus",
            }));
        };
        let label = comparison_label(&range);
        let view = match view {
            RangeView::Commit { sha } => {
                let listed = a.snapshotter.current().await?;
                let Some(found) = listed.commits.list.iter().find(|c| c.sha == sha || c.short == sha) else {
                    return Err(Error::revspec(format!("not a listed commit of {label}: {sha}")));
                };
                RangeView::Commit { sha: found.sha.clone() }
            }
            RangeView::Interdiff { from, to, pair } => {
                if let (RangeRef::Iteration { n: older }, Some(RangeRef::Iteration { n: newer })) = (&from, &to)
                    && older >= newer
                {
                    return Err(Error::revspec("compare an older iteration with a newer one"));
                }
                let pair = match pair {
                    None => None,
                    Some(sha) => {
                        let shown = a.snapshotter.current().await?;
                        let full = shown.interdiff.as_ref().and_then(|inter| {
                            inter.pairs.iter().find_map(|p| {
                                p.new.as_ref().filter(|n| n.sha == sha || n.short == sha).map(|n| n.sha.clone())
                            })
                        });
                        Some(full.unwrap_or(sha))
                    }
                };
                RangeView::Interdiff { from, to, pair }
            }
            view => view,
        };
        Ok(with_view(&range, view).expect("a range"))
    }
}
