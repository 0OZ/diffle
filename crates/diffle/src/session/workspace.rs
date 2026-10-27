//! The owned checkout's read leases and before-mutation lifecycle boundary.
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use futures::future::BoxFuture;
use tokio::sync::{OwnedRwLockReadGuard, RwLock};

use super::Session;
use crate::error::{Error, Result};
use crate::git::WORKTREE;
use crate::protocol::Snapshot;

type BeforeCheckout = Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>;
type Restored = Arc<dyn Fn(Arc<Snapshot>) -> BoxFuture<'static, ()> + Send + Sync>;

#[derive(Default)]
pub(super) struct Workspace {
    pub(super) gate: Arc<RwLock<()>>,
    before_checkout: Mutex<Option<BeforeCheckout>>,
    restored: Mutex<Option<Restored>>,
    fault: Mutex<Option<Error>>,
}

impl Workspace {
    pub(super) fn check(&self) -> Result<()> {
        match self.fault.lock().unwrap().clone() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Session {
    /// Holds the checkout stable through snapshot validation and an entire language-server query.
    pub async fn workspace_read(&self) -> Result<OwnedRwLockReadGuard<()>> {
        let guard = self.workspace.gate.clone().read_owned().await;
        self.workspace.check()?;
        Ok(guard)
    }

    /// Installs the lifecycle hook before starting the session. It must reap old servers before returning.
    pub fn before_checkout(&self, callback: impl Fn() -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        *self.workspace.before_checkout.lock().unwrap() = Some(Arc::new(callback));
    }

    /// Rebuilds services against the still-published snapshot after a failed checkout transaction.
    pub fn on_checkout_restored(
        &self,
        callback: impl Fn(Arc<Snapshot>) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    ) {
        *self.workspace.restored.lock().unwrap() = Some(Arc::new(callback));
    }

    pub(super) async fn resume_workspace_services(&self, snap: Arc<Snapshot>) {
        let callback = self.workspace.restored.lock().unwrap().clone();
        if let Some(callback) = callback {
            callback(snap).await;
        }
    }

    /// A tracking job checks this after acquiring its read lease, not when it was queued.
    pub async fn is_current_snapshot(&self, snap: &Arc<Snapshot>) -> bool {
        let Ok(snapshotter) = self.snapshotter() else { return false };
        snapshotter.current().await.is_ok_and(|current| Arc::ptr_eq(&current, snap))
    }

    pub(super) async fn reset_workspace_services(&self) {
        let callback = self.workspace.before_checkout.lock().unwrap().clone();
        if let Some(callback) = callback {
            callback().await;
        }
    }

    /// A dirty worktree view may be reviewed, but leaving it must not silently discard edits.
    pub(super) async fn checkout_previous(&self, target: &str) -> Result<Option<String>> {
        if !self.repo.is_isolated() || target == WORKTREE {
            return Ok(None);
        }
        let previous = self.repo.resolve("HEAD").await?;
        if !self.repo.numstat("HEAD", WORKTREE).await?.is_empty() {
            return Err(Error::revspec("commit or discard the isolated working changes before changing revisions"));
        }
        Ok((previous != target).then_some(previous))
    }

    /// Called under the transition write lease. A rollback failure disables queries rather than serving mismatched disk.
    pub(super) async fn restore_checkout(&self, previous: &str, failed: &Error) -> Result<()> {
        self.reset_workspace_services().await;
        if self.repo.resolve("HEAD").await.is_ok_and(|head| head == previous)
            && self.repo.numstat("HEAD", WORKTREE).await.is_ok_and(|changes| changes.is_empty())
        {
            return Ok(());
        }
        if let Err(rollback) = self.repo.checkout_owned(previous).await {
            let error: Error = anyhow::anyhow!("{failed}; restoring isolated checkout also failed: {rollback}").into();
            *self.workspace.fault.lock().unwrap() = Some(error.clone());
            self.active.lock().unwrap().take();
            self.ready.send_replace(Some(Err(error.clone())));
            return Err(error);
        }
        Ok(())
    }

    /// Queued tracking holds a read lease, so it cannot overlap a checkout or revive a stale snapshot.
    pub fn track_snapshot(
        self: &Arc<Self>,
        snap: Arc<Snapshot>,
        track: impl FnOnce(Arc<Snapshot>) -> BoxFuture<'static, ()> + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let session = Arc::downgrade(self);
        tokio::spawn(
            async move {
                let Some(session) = session.upgrade() else { return };
                let Ok(_guard) = session.workspace_read().await else { return };
                if session.is_current_snapshot(&snap).await {
                    track(snap).await;
                }
            }
            .boxed(),
        )
    }
}
