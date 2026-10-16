//! Owns the active mode and everything derived from it.
//!
//! Every mutation of the active state (mode transition, refresh, context change) runs through
//! one queue, one at a time, in request order: the last mode requested ends up active with the
//! only live watcher, a snapshot version is allocated only while its task holds the queue, so
//! published versions are monotonic and the active snapshotter always carries the session's
//! context.
//!
//! A worktree-live comparison recomputes on every change. A refs-live one only announces that
//! its refs moved and recomputes on `reload`, so a push or rebase does not swap the review out
//! from under the reviewer; the follow-refs setting decides.

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, PoisonError, RwLock};

use tokio::sync::{Mutex, broadcast, watch};

use crate::domains::comments::storage::comment_store::CommentStore;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::ignored_paths::IgnoredPaths;
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::session::session_watch::{Watch, WatcherFactory};
use crate::domains::review::snapshot::review_snapshot::{Moved, Snapshot};
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;
use crate::domains::review::snapshot::snapshotter::Snapshotter;
use crate::domains::revisions::comparison::Liveness;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

/// Events a slow subscriber may lag behind before it misses some. Every event only says
/// "refetch", so a miss costs nothing a later event does not repair.
const EVENT_BACKLOG: usize = 64;

/// What the program decides about a session before it starts.
pub struct SessionConfig {
    pub context: ContextLines,
    pub follow_refs: FollowRefs,
    /// `None`: nothing watches, and the review changes only when asked.
    pub watcher: Option<WatcherFactory>,
}

/// The settings a running session lets the reviewer change.
#[derive(Debug, Clone, Copy)]
pub(super) struct LiveSettings {
    pub context: ContextLines,
    pub follow_refs: FollowRefs,
}

/// Held for the length of one transition.
pub(super) struct Queue {
    version: SnapshotVersion,
    /// The newest refresh request a recompute has covered; a queued refresh at or below it has
    /// nothing left to do.
    pub(super) refreshed_through: u64,
    /// The same for checks of where the followed refs point.
    pub(super) checked_through: u64,
}

impl Queue {
    /// The next version, taken for the transition that holds the queue.
    pub(super) fn allocate(&mut self) -> SnapshotVersion {
        self.version = self.version.next();
        self.version
    }
}

/// The state of one open review. Replaced whole on a mode switch, so a reader holding an
/// `Arc<Active>` sees one consistent mode, snapshotter and comment store.
pub struct Active {
    pub(super) mode: ReviewMode,
    pub(super) snapshotter: Snapshotter,
    pub(super) comments: CommentStore,
    /// Where the followed refs point now, when that differs from the snapshot; cleared by a reload.
    pub(super) moved: std::sync::Mutex<Option<Moved>>,
    /// Whether an endpoint of the followed range names a branch; decides what `auto` does with a move.
    pub(super) names_branch: bool,
    /// What a worktree watcher must not wake for; refreshed after each recompute.
    pub(super) ignored: IgnoredPaths,
    pub(super) watch: Mutex<Option<Box<dyn Send + Sync>>>,
    /// Set once this review has been replaced or the session closed; ends the task that answers
    /// the watcher, whether or not the watcher closes its channel.
    pub(super) retired: watch::Sender<bool>,
}

impl Active {
    #[must_use]
    pub const fn mode(&self) -> &ReviewMode {
        &self.mode
    }

    #[must_use]
    pub const fn snapshotter(&self) -> &Snapshotter {
        &self.snapshotter
    }

    #[must_use]
    pub fn comments(&self) -> CommentStore {
        self.comments.clone()
    }

    /// The pending reload of a refs-live comparison, for a client that connects after the push.
    #[must_use]
    pub fn moved(&self) -> Option<Moved> {
        self.moved
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(super) fn set_moved(&self, moved: Option<Moved>) {
        *self.moved.lock().unwrap_or_else(PoisonError::into_inner) = moved;
    }

    /// Ends the watch; no dirty signal arrives afterwards.
    pub(super) async fn stop_watching(&self) {
        self.retired.send_replace(true);
        drop(self.watch.lock().await.take());
    }

    pub(super) async fn keep_watching(&self, watch: Watch) -> tokio::sync::mpsc::Receiver<()> {
        *self.watch.lock().await = Some(watch.stop);
        watch.dirty
    }
}

pub struct Session {
    pub(super) repo: Arc<GitRepo>,
    pub(super) events: broadcast::Sender<SessionEvent>,
    pub(super) latest: watch::Sender<Option<Arc<Snapshot>>>,
    pub(super) watcher: Option<WatcherFactory>,
    /// Refresh and check requests made so far; each queued request carries its number.
    pub(super) refresh_requests: AtomicU64,
    pub(super) check_requests: AtomicU64,
    pub(super) queue: Mutex<Queue>,
    pub(super) active: RwLock<Option<Arc<Active>>>,
    pub(super) settings: RwLock<LiveSettings>,
}

impl Session {
    #[must_use]
    pub fn new(repo: Arc<GitRepo>, config: SessionConfig) -> Arc<Self> {
        Arc::new(Self {
            repo,
            events: broadcast::channel(EVENT_BACKLOG).0,
            latest: watch::channel(None).0,
            watcher: config.watcher,
            refresh_requests: AtomicU64::new(0),
            check_requests: AtomicU64::new(0),
            queue: Mutex::new(Queue {
                version: SnapshotVersion::default(),
                refreshed_through: 0,
                checked_through: 0,
            }),
            active: RwLock::new(None),
            settings: RwLock::new(LiveSettings {
                context: config.context,
                follow_refs: config.follow_refs,
            }),
        })
    }

    #[must_use]
    pub fn repo(&self) -> &Arc<GitRepo> {
        &self.repo
    }

    /// Changes of the review, as they happen.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    /// The newest snapshot the session produced, for consumers that follow it (language servers).
    #[must_use]
    pub fn follow_snapshots(&self) -> watch::Receiver<Option<Arc<Snapshot>>> {
        self.latest.subscribe()
    }

    /// The open review, or an error before the first mode started.
    pub fn active(&self) -> Result<Arc<Active>, ReviewError> {
        self.active
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(ReviewError::NotOpen)
    }

    /// Context lines the session generates patches with.
    #[must_use]
    pub fn context(&self) -> ContextLines {
        self.live_settings().context
    }

    /// Changes the user's choice for the running session; a pending notice stands until a
    /// reload picks the refs up.
    pub fn set_follow_refs(&self, follow_refs: FollowRefs) {
        self.settings
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .follow_refs = follow_refs;
    }

    /// Whether moved refs recompute the review, as worktree edits always do, or only announce
    /// themselves.
    #[must_use]
    pub fn follows(&self, active: &Active) -> bool {
        match active.mode.comparison().live() {
            Liveness::Worktree => true,
            Liveness::Pinned => false,
            Liveness::Refs => self
                .live_settings()
                .follow_refs
                .recomputes(active.names_branch),
        }
    }

    pub(super) fn live_settings(&self) -> LiveSettings {
        *self.settings.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn is_active(&self, candidate: &Arc<Active>) -> bool {
        self.active
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, candidate))
    }

    /// Waits for the running transition, then ends the watch.
    pub async fn close(&self) {
        let _settled = self.queue.lock().await;
        if let Ok(active) = self.active() {
            active.stop_watching().await;
        }
    }
}
