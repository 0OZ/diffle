//! The one task that owns the OS watcher and the debounce clock.

use std::path::PathBuf;
use std::sync::Arc;

use notify::RecommendedWatcher;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::mpsc::{Sender, UnboundedReceiver};
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep_until};

use super::debounce::Debouncer;
use super::observation::Observation;
use super::scope::event_filter::EventFilter;
use super::tree_watch::watch_checkout_tree;

pub struct EventLoop {
    /// Held so the OS watches live as long as the loop; taken only while new directories
    /// are being armed on a blocking thread.
    watcher: Option<RecommendedWatcher>,
    filter: Arc<EventFilter>,
    observations: UnboundedReceiver<Observation>,
    signals: Sender<()>,
    debounce: Debouncer,
    shutdown: oneshot::Receiver<()>,
}

impl EventLoop {
    #[must_use]
    pub fn new(
        watcher: RecommendedWatcher,
        filter: Arc<EventFilter>,
        observations: UnboundedReceiver<Observation>,
        signals: Sender<()>,
        debounce: Debouncer,
        shutdown: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            watcher: Some(watcher),
            filter,
            observations,
            signals,
            debounce,
            shutdown,
        }
    }

    /// Runs until shutdown is requested (or its sender dropped), the consumer drops the
    /// receiver, or the OS watcher is lost. Dropping the loop stops the OS watches.
    pub async fn run(mut self) {
        loop {
            let due = self.debounce.deadline();
            tokio::select! {
                // Shutdown first, then queued events, so a burst folds in before the clock fires.
                biased;
                _ = &mut self.shutdown => return,
                observation = self.observations.recv() => {
                    let Some(observation) = observation else { return };
                    self.debounce.touch(Instant::now());
                    if !observation.appeared.is_empty() && !self.arm(observation.appeared).await {
                        return;
                    }
                }
                () = until(due) => {
                    self.debounce.settle();
                    if !self.deliver() {
                        return;
                    }
                }
            }
        }
    }

    /// One pending signal is all a consumer needs: when it still holds one, this change is
    /// already announced, so the signal is dropped rather than queued or awaited.
    /// Returns false once the consumer is gone.
    fn deliver(&self) -> bool {
        match self.signals.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => true,
            Err(TrySendError::Closed(())) => false,
        }
    }

    /// Watches the directories among `appeared`, off the runtime's threads: a new tree can
    /// be large. Returns false when the watcher was lost with the blocking task.
    async fn arm(&mut self, appeared: Vec<PathBuf>) -> bool {
        let Some(mut watcher) = self.watcher.take() else {
            return false;
        };
        let filter = Arc::clone(&self.filter);
        let armed = tokio::task::spawn_blocking(move || {
            for path in &appeared {
                if let Err(error) = watch_checkout_tree(&mut watcher, path, &filter) {
                    tracing::warn!(%error, "new directory left unwatched");
                }
            }
            watcher
        })
        .await;
        match armed {
            Ok(watcher) => {
                self.watcher = Some(watcher);
                true
            }
            Err(error) => {
                tracing::warn!(%error, "file-system watcher lost while arming a new directory");
                false
            }
        }
    }
}

/// Resolves at `deadline`; never when there is none.
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => sleep_until(at).await,
        None => std::future::pending().await,
    }
}
