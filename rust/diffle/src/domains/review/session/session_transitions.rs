//! The session's state changes. Each runs alone, in request order, on a task of its own, so a
//! caller that gives up (a dropped connection) cannot leave a transition half done.

use std::future::Future;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use tokio::sync::{Mutex, watch};

use crate::domains::comments::storage::comment_store::CommentStore;
use crate::domains::comments::storage::storage_key::StorageKey;
use crate::domains::review::mode::comparison_label::comparison_label;
use crate::domains::review::mode::mode_request::{FocusTarget, ModeRequest};
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::ignored_paths::IgnoredPaths;
use crate::domains::review::session::review_session::{Active, Queue, Session};
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::session::session_live::names_a_branch;
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::review::snapshot::snapshotter::Snapshotter;
use crate::domains::revisions::comparison::Comparison;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::resolution::resolve_review::resolve_review;
use crate::domains::settings::context_lines::ContextLines;

impl Session {
    /// Runs `job` to its end on a task of its own, whatever happens to the caller.
    pub(super) async fn detached<T, Fut>(
        self: &Arc<Self>,
        job: impl FnOnce(Arc<Self>) -> Fut,
    ) -> Result<T, ReviewError>
    where
        Fut: Future<Output = Result<T, ReviewError>> + Send + 'static,
        T: Send + 'static,
    {
        tokio::spawn(job(Arc::clone(self)))
            .await
            .map_err(|_| ReviewError::Interrupted)?
    }

    /// Tells every subscriber. The session announces its own snapshots; whoever changes
    /// threads, marks or the config announces those.
    pub fn announce(&self, event: SessionEvent) {
        // Nobody listening is not a failure.
        let _ = self.events.send(event);
    }

    /// Turns what the reviewer asked for into a comparison without activating it, so bad input
    /// is rejected before anything is served.
    pub async fn resolve(&self, request: &EntryRequest) -> Result<Comparison, ReviewError> {
        Ok(resolve_review(&self.repo, request).await?)
    }

    /// Opens the first review. Clients are not told: none can be connected yet.
    pub async fn start(
        self: &Arc<Self>,
        comparison: Comparison,
    ) -> Result<Arc<Snapshot>, ReviewError> {
        self.detached(|session| async move {
            let mut queue = session.queue.lock().await;
            session
                .activate(&mut queue, ReviewMode::whole(comparison))
                .await
        })
        .await
    }

    /// Switches what is shown and tells clients to refetch.
    pub async fn switch_mode(
        self: &Arc<Self>,
        request: ModeRequest,
    ) -> Result<Arc<Snapshot>, ReviewError> {
        self.detached(|session| async move {
            let mut queue = session.queue.lock().await;
            let mode = match request {
                ModeRequest::Enter(entry) => {
                    ReviewMode::whole(resolve_review(&session.repo, &entry).await?)
                }
                ModeRequest::Focus(target) => session.focused(target).await?,
            };
            session.enter(&mut queue, mode).await
        })
        .await
    }

    /// Changes the context lines of every patch; clients refetch them.
    pub async fn set_context(self: &Arc<Self>, context: ContextLines) -> Result<(), ReviewError> {
        self.detached(|session| async move {
            let mut queue = session.queue.lock().await;
            if session.context() == context {
                return Ok(());
            }
            session
                .settings
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .context = context;
            let Ok(active) = session.active() else {
                return Ok(());
            };
            active.snapshotter.set_context(context, queue.allocate());
            session.publish(&active).await.map(drop)
        })
        .await
    }

    /// Recomputes the review after a change. A burst of requests costs the recompute that is
    /// running and one more that sees all of them. A failure is logged: the next change retries.
    pub async fn refresh(self: &Arc<Self>) {
        let ticket = self.refresh_requests.fetch_add(1, Ordering::AcqRel) + 1;
        let outcome = self
            .detached(|session| async move {
                let mut queue = session.queue.lock().await;
                if queue.refreshed_through >= ticket {
                    return Ok(());
                }
                queue.refreshed_through = session.refresh_requests.load(Ordering::Acquire);
                let active = session.active()?;
                session.recompute(&mut queue, &active).await.map(drop)
            })
            .await;
        if let Err(error) = outcome {
            tracing::error!(%error, "refresh failed");
        }
    }

    /// The reviewer asks for the moved refs: recompute now. Unlike `refresh`, a failure is theirs.
    pub async fn reload(self: &Arc<Self>) -> Result<Arc<Snapshot>, ReviewError> {
        self.detached(|session| async move {
            let mut queue = session.queue.lock().await;
            let active = session.active()?;
            session.recompute(&mut queue, &active).await
        })
        .await
    }

    /// The mode a focus request leads to. A focused commit keeps its own comments; the
    /// uncommitted changes share `working`'s.
    async fn focused(&self, target: FocusTarget) -> Result<ReviewMode, ReviewError> {
        let active = self.active()?;
        if active.mode.within().is_none() && active.mode.is_single_commit() {
            return Err(ReviewError::NothingToFocus);
        }
        let range = active.mode.range().clone();
        match target {
            FocusTarget::Range => Ok(ReviewMode::whole(range)),
            FocusTarget::Worktree => match range.spec().ends_at_worktree() {
                true => Ok(ReviewMode::worktree_within(range)),
                false => Err(ReviewError::NoUncommittedChanges {
                    range: comparison_label(&range),
                }),
            },
            FocusTarget::Commit(wanted) => {
                let snapshot = active.snapshotter.current().await?;
                let listed = &snapshot.parts().commits.commits.list;
                let found = listed.iter().find(|commit| {
                    commit.info.id.as_str() == wanted || commit.info.short == wanted
                });
                match found {
                    Some(commit) => Ok(ReviewMode::commit_within(&commit.info.id, range)),
                    None => Err(ReviewError::NotListed {
                        range: comparison_label(&range),
                        commit: wanted,
                    }),
                }
            }
        }
    }

    /// Activates `mode` and tells clients.
    async fn enter(
        self: &Arc<Self>,
        queue: &mut Queue,
        mode: ReviewMode,
    ) -> Result<Arc<Snapshot>, ReviewError> {
        let snapshot = self.activate(queue, mode).await?;
        self.announce(SessionEvent::Snapshot(snapshot.version()));
        Ok(snapshot)
    }

    /// Builds the whole next state, then swaps it in and retires the previous one.
    async fn activate(
        self: &Arc<Self>,
        queue: &mut Queue,
        mode: ReviewMode,
    ) -> Result<Arc<Snapshot>, ReviewError> {
        let snapshotter = Snapshotter::new(
            Arc::clone(&self.repo),
            mode.clone(),
            queue.allocate(),
            self.context(),
        );
        let key = StorageKey::new(mode.comparison().comment_key().as_str());
        let (comments, snapshot, names_branch) = tokio::try_join!(
            async {
                CommentStore::open(self.repo.git_dir(), &key)
                    .await
                    .map_err(ReviewError::from)
            },
            snapshotter.current(),
            async {
                names_a_branch(&self.repo, &mode)
                    .await
                    .map_err(ReviewError::from)
            },
        )?;
        let next = Arc::new(Active {
            mode,
            snapshotter,
            comments,
            moved: std::sync::Mutex::new(None),
            names_branch,
            ignored: IgnoredPaths::default(),
            watch: Mutex::new(None),
            retired: watch::channel(false).0,
        });
        // The repository may have moved on while no server was watching it.
        self.relocate_comments(&next, &snapshot).await?;
        self.watch(&next).await?;
        let previous = self
            .active
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(Arc::clone(&next));
        if let Some(previous) = previous {
            previous.stop_watching().await;
        }
        self.latest.send_replace(Some(Arc::clone(&snapshot)));
        Ok(snapshot)
    }

    async fn recompute(
        &self,
        queue: &mut Queue,
        active: &Arc<Active>,
    ) -> Result<Arc<Snapshot>, ReviewError> {
        active.snapshotter.invalidate(queue.allocate());
        self.publish(active).await
    }

    /// Computes `active`'s current snapshot, re-anchors its threads (hunks may have grown,
    /// shrunk or moved), then tells clients and followers.
    async fn publish(&self, active: &Active) -> Result<Arc<Snapshot>, ReviewError> {
        let snapshot = active.snapshotter.current().await?;
        self.relocate_comments(active, &snapshot).await?;
        // The snapshot resolved the refs afresh; a signal arriving later compares against it.
        active.set_moved(None);
        self.announce(SessionEvent::Snapshot(snapshot.version()));
        self.latest.send_replace(Some(Arc::clone(&snapshot)));
        Ok(snapshot)
    }
}
