//! How the session asks for file-system changes without knowing who watches.

use std::path::PathBuf;
use std::sync::Arc;

use futures::future::BoxFuture;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::domains::review::session::ignored_paths::IgnoredPaths;

/// What to observe for a live comparison.
#[derive(Debug, Clone)]
pub enum WatchRequest {
    /// The checkout and the Git metadata that decides its changes.
    Worktree {
        root: PathBuf,
        git_dir: PathBuf,
        common_dir: PathBuf,
        /// Kept current by the session; a watcher must not wake for what it covers.
        ignored: IgnoredPaths,
    },
    /// Only the refs and `HEAD` that move the old side.
    Refs {
        git_dir: PathBuf,
        common_dir: PathBuf,
    },
}

#[derive(Debug, Error)]
#[error("cannot watch for changes: {0}")]
pub struct WatchFailure(pub String);

/// A running watch. Dropping `stop` ends it, and `dirty` then closes.
pub struct Watch {
    /// One message after a burst of changes settled.
    pub dirty: mpsc::Receiver<()>,
    pub stop: Box<dyn Send + Sync>,
}

/// Starts watching and returns once it is observing. Supplied by the program that runs the
/// session, since the session itself touches no file-system notification API.
pub type WatcherFactory =
    Arc<dyn Fn(WatchRequest) -> BoxFuture<'static, Result<Watch, WatchFailure>> + Send + Sync>;
