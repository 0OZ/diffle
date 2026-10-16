//! Putting OS watches on paths, with the failure policy of each kind.

use std::path::Path;

use notify::{ErrorKind, RecursiveMode, Watcher};

use super::error::WatchError;
use super::scope::checkout_walk::watchable_dirs;
use super::scope::event_filter::EventFilter;

/// Watches one path that must be observable, or says why not (missing, unreadable, out of
/// watches).
pub fn watch_required(
    watcher: &mut impl Watcher,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatchError> {
    watcher
        .watch(path, mode)
        .map_err(|source| WatchError::Watch {
            path: path.to_path_buf(),
            source,
        })
}

/// Watches `start` and the checkout directories beneath it, each non-recursively. A
/// directory that vanished or cannot be read is skipped: it is a race, not a failure. Running
/// out of OS watches is reported, because the checkout would then be half-observed.
pub fn watch_checkout_tree(
    watcher: &mut impl Watcher,
    start: &Path,
    filter: &EventFilter,
) -> Result<(), WatchError> {
    for dir in watchable_dirs(start, filter) {
        match watcher.watch(&dir, RecursiveMode::NonRecursive) {
            Ok(()) => {}
            Err(source) if matches!(source.kind, ErrorKind::MaxFilesWatch) => {
                return Err(WatchError::Watch { path: dir, source });
            }
            Err(source) => {
                tracing::debug!(path = %dir.display(), %source, "skipped an unwatchable directory");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tree_watch_tests.rs"]
mod tests;
