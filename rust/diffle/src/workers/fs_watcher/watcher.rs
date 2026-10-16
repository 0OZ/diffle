//! The handle a session holds on its file-system watcher.

use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::debounce::Debouncer;
use super::error::WatchError;
use super::event_loop::EventLoop;
use super::observation::{Observation, observe};
use super::scope::event_filter::EventFilter;
use super::scope::watch_target::WatchTarget;
use super::tree_watch::{watch_checkout_tree, watch_required};

/// Quiet time after the last change before the signal fires.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(150);

/// One undelivered signal is all a slow consumer ever needs to learn "something changed".
const SIGNAL_CAPACITY: usize = 1;

/// Observes a [`WatchTarget`] and announces changes as one signal per burst.
pub struct FsWatcher {
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl FsWatcher {
    /// Starts observing and returns once the watches are in place: a change made after
    /// this returns is seen. After `debounce` of quiet following any change, at least one
    /// `()` arrives on the receiver; a burst yields one, and a consumer that lags holds at
    /// most one pending signal without ever slowing the watcher.
    ///
    /// Fails when a path that must be watched is missing or unwatchable, or the OS is out
    /// of watches. A path that vanishes later is tolerated.
    pub async fn start(
        target: WatchTarget,
        debounce: Duration,
    ) -> Result<(Self, mpsc::Receiver<()>), WatchError> {
        let filter = Arc::new(EventFilter::new(&target.absolute()?));
        let (observations_tx, observations) = mpsc::unbounded_channel();
        let armed = {
            let filter = Arc::clone(&filter);
            tokio::task::spawn_blocking(move || arm(&filter, observations_tx)).await
        };
        let watcher = armed.map_err(WatchError::Setup)??;

        let (signals, receiver) = mpsc::channel(SIGNAL_CAPACITY);
        let (shutdown, shutdown_rx) = oneshot::channel();
        let event_loop = EventLoop::new(
            watcher,
            filter,
            observations,
            signals,
            Debouncer::new(debounce),
            shutdown_rx,
        );
        let task = tokio::spawn(event_loop.run());
        Ok((Self { shutdown, task }, receiver))
    }

    /// Stops observing and waits until the watcher task has ended: nothing is signalled
    /// afterwards, though a signal already queued stays in the receiver. The receiver then
    /// yields `None`.
    pub async fn close(self) {
        // A task that already ended has dropped its receiver; there is nothing to tell.
        let _ = self.shutdown.send(());
        if let Err(error) = self.task.await {
            tracing::warn!(%error, "file-system watcher task ended abnormally");
        }
    }
}

/// Creates the OS watcher and puts every watch in place, on a blocking thread because it
/// walks the checkout.
fn arm(
    filter: &Arc<EventFilter>,
    observations: UnboundedSender<Observation>,
) -> Result<RecommendedWatcher, WatchError> {
    let on_event = {
        let filter = Arc::clone(filter);
        // Runs on the OS watcher's own thread: filter there, so only changes that matter
        // cross to the task. A closed channel means the watcher is shutting down.
        move |result: notify::Result<notify::Event>| match result {
            Ok(event) => {
                if let Some(observation) = observe(&event, &filter) {
                    let _ = observations.send(observation);
                }
            }
            Err(error) => tracing::warn!(%error, "file-system watcher error"),
        }
    };
    let mut watcher = notify::recommended_watcher(on_event).map_err(WatchError::Create)?;

    let metadata = filter.metadata();
    for git_dir in metadata.git_dirs() {
        watch_required(&mut watcher, git_dir, RecursiveMode::NonRecursive)?;
    }
    watch_required(&mut watcher, metadata.refs_dir(), RecursiveMode::Recursive)?;
    if let Some(root) = filter.checkout_root() {
        watch_required(&mut watcher, root, RecursiveMode::NonRecursive)?;
        watch_checkout_tree(&mut watcher, root, filter)?;
    }
    Ok(watcher)
}

#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "watcher_git_tests.rs"]
mod git_tests;
