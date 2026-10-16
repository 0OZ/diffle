//! The session as the repository changes under it: watching, recomputing on a change, and
//! noticing that followed refs moved.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use tokio::sync::{mpsc, watch};

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::review_session::{Active, Session};
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::session::session_watch::WatchRequest;
use crate::domains::review::snapshot::review_snapshot::Moved;
use crate::domains::revisions::comparison::Liveness;
use crate::domains::revisions::resolution::resolve_endpoints::resolve_endpoints;

impl Session {
    /// What `active` needs watched, if a watcher was supplied and the review can change. Reads
    /// Git first, so a failure here leaves the review that is open as it was.
    pub(super) async fn watch_request(
        &self,
        active: &Active,
    ) -> Result<Option<WatchRequest>, ReviewError> {
        if self.watcher.is_none() {
            return Ok(None);
        }
        let (git_dir, common_dir) = (
            self.repo.git_dir().to_path_buf(),
            self.repo.common_dir().to_path_buf(),
        );
        Ok(match active.mode.comparison().live() {
            Liveness::Pinned => None,
            Liveness::Worktree => {
                self.refresh_ignored(active).await?;
                Some(WatchRequest::Worktree {
                    root: self.repo.root().to_path_buf(),
                    git_dir,
                    common_dir,
                    ignored: active.ignored.clone(),
                })
            }
            Liveness::Refs => Some(WatchRequest::Refs {
                git_dir,
                common_dir,
            }),
        })
    }

    /// Starts the watch `request` describes. A watcher that cannot start (the OS ran out of
    /// watches) leaves the review open without live updates: it still shows the diff, and the
    /// reviewer can reload.
    pub(super) async fn arm(self: &Arc<Self>, active: &Arc<Active>, request: Option<WatchRequest>) {
        let (Some(request), Some(factory)) = (request, &self.watcher) else {
            return;
        };
        match factory(request).await {
            Ok(watch) => {
                let dirty = active.keep_watching(watch).await;
                tokio::spawn(follow_changes(
                    Arc::downgrade(self),
                    Arc::downgrade(active),
                    dirty,
                    active.retired.subscribe(),
                ));
            }
            Err(failure) => {
                tracing::warn!(%failure, "continuing without live updates");
                active.set_unwatched(failure.to_string());
            }
        }
    }

    pub(super) async fn refresh_ignored(&self, active: &Active) -> Result<(), GitError> {
        active.ignored.replace(self.repo.ignored_entries().await?);
        Ok(())
    }

    /// Resolves the refs the live comparison follows and tells clients when they left the
    /// snapshot behind, or came back to it. As with `refresh`, a burst of signals costs one
    /// check after the running one.
    pub(super) async fn check_moved(self: &Arc<Self>, active: &Arc<Active>) {
        let ticket = self.check_requests.fetch_add(1, Ordering::AcqRel) + 1;
        let active = Arc::clone(active);
        let outcome = self
            .detached(move |session| async move {
                let mut queue = session.queue.lock().await;
                if queue.checked_through >= ticket {
                    return Ok(());
                }
                queue.checked_through = session.check_requests.load(Ordering::Acquire);
                if !session.is_active(&active) {
                    return Ok(());
                }
                session.note_moved(&active).await
            })
            .await;
        if let Err(error) = outcome {
            tracing::error!(%error, "refs check failed");
        }
    }

    async fn note_moved(&self, active: &Active) -> Result<(), ReviewError> {
        let snapshot = active.snapshotter.current().await?;
        let now = resolve_endpoints(&self.repo, active.mode.range().spec()).await?;
        let shown = &snapshot.parts().commits;
        let moved = (now.old != shown.old || now.new != shown.new).then(|| Moved {
            version: snapshot.version(),
            old: now.old,
            new: now.new,
        });
        let sides = |moved: &Option<Moved>| moved.as_ref().map(|m| (m.old.clone(), m.new.clone()));
        if sides(&moved) == sides(&active.moved()) {
            return Ok(());
        }
        active.set_moved(moved.clone());
        self.announce(SessionEvent::Moved(moved));
        Ok(())
    }

    /// Re-reads what Git ignores, inside the queue so `close` waits for its process.
    async fn refresh_ignored_queued(
        self: &Arc<Self>,
        active: &Arc<Active>,
    ) -> Result<(), ReviewError> {
        let active = Arc::clone(active);
        self.detached(move |session| async move {
            let _turn = session.queue.lock().await;
            if session.is_active(&active) && active.mode.comparison().live() == Liveness::Worktree {
                session.refresh_ignored(&active).await?;
            }
            Ok(())
        })
        .await
    }
}

/// Answers each settled burst of changes until the review is retired or the watch ends.
async fn follow_changes(
    session: Weak<Session>,
    active: Weak<Active>,
    mut dirty: mpsc::Receiver<()>,
    mut retired: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            signal = dirty.recv() => if signal.is_none() { return },
            // A closed channel means the review is gone, which retires it just the same.
            _ = retired.wait_for(|retired| *retired) => return,
        }
        let (Some(session), Some(active)) = (session.upgrade(), active.upgrade()) else {
            return;
        };
        if !session.is_active(&active) {
            return;
        }
        match session.follows(&active) {
            false => session.check_moved(&active).await,
            true => {
                session.refresh().await;
                if let Err(error) = session.refresh_ignored_queued(&active).await {
                    tracing::warn!(%error, "ignored paths refresh failed");
                }
            }
        }
    }
}

/// Whether either endpoint of the followed range names a branch (a local or remote one, or a
/// `HEAD` on one), which is what `auto` follow-refs decides by.
pub(super) async fn names_a_branch(repo: &GitRepo, mode: &ReviewMode) -> Result<bool, GitError> {
    let range = mode.range();
    if range.live() != Liveness::Refs {
        return Ok(false);
    }
    let names = range.spec().named_revisions();
    let found =
        futures::future::try_join_all(names.into_iter().map(|name| repo.branch_ref(name))).await?;
    Ok(found.iter().any(Option::is_some))
}
