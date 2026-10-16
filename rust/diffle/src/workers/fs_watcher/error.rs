//! Why a watcher could not start.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatchError {
    /// A watch path could not be made absolute (no current directory to resolve it against).
    #[error("cannot resolve the watch path {}: {source}", path.display())]
    Resolve { path: PathBuf, source: io::Error },
    /// The OS watcher itself could not be created.
    #[error("cannot create the file-system watcher: {0}")]
    Create(#[source] notify::Error),
    /// A path that must be observed could not be: missing, unreadable, or the OS ran out
    /// of watches (inotify's limit).
    #[error("cannot watch {}: {source}", path.display())]
    Watch {
        path: PathBuf,
        source: notify::Error,
    },
    /// The thread that arms the watches died before finishing.
    #[error("the watcher setup task failed: {0}")]
    Setup(#[source] tokio::task::JoinError),
}
