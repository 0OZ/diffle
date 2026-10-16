//! The file-system watcher, offered to the session through the port it asks for.

use std::sync::Arc;

use crate::domains::review::session::session_watch::{
    Watch, WatchFailure, WatchRequest, WatcherFactory,
};
use crate::workers::fs_watcher::scope::watch_target::WatchTarget;
use crate::workers::fs_watcher::watcher::{DEFAULT_DEBOUNCE, FsWatcher};

/// A factory that starts a real [`FsWatcher`] for whatever the session wants watched.
#[must_use]
pub fn session_watchers() -> WatcherFactory {
    Arc::new(|request| Box::pin(start(request)))
}

async fn start(request: WatchRequest) -> Result<Watch, WatchFailure> {
    let (watcher, dirty) = FsWatcher::start(target_of(request), DEFAULT_DEBOUNCE)
        .await
        .map_err(|error| WatchFailure(error.explained()))?;
    // Dropping the watcher ends it, which is how the session stops a watch.
    Ok(Watch {
        dirty,
        stop: Box::new(watcher),
    })
}

fn target_of(request: WatchRequest) -> WatchTarget {
    match request {
        WatchRequest::Worktree {
            root,
            git_dir,
            common_dir,
            ignored,
        } => WatchTarget::Worktree {
            root,
            git_dir,
            common_dir,
            ignored: Arc::new(move |path| ignored.covers(path)),
        },
        WatchRequest::Refs {
            git_dir,
            common_dir,
        } => WatchTarget::Refs {
            git_dir,
            common_dir,
        },
    }
}

#[cfg(test)]
#[path = "session_watchers_tests.rs"]
mod tests;
