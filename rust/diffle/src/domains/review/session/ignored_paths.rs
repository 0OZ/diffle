//! The paths Git ignores, kept current for a watcher that must not wake for them.

use std::collections::HashSet;
use std::sync::{Arc, PoisonError, RwLock};

const DIRECTORY_MARK: char = '/';

/// Repository-relative ignored paths: files, and directories with a trailing `/`. Clones share
/// one set, so the session can refresh it while a watcher keeps asking.
#[derive(Debug, Clone, Default)]
pub struct IgnoredPaths(Arc<RwLock<HashSet<String>>>);

impl IgnoredPaths {
    pub fn replace(&self, entries: impl IntoIterator<Item = String>) {
        let next: HashSet<String> = entries.into_iter().collect();
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = next;
    }

    /// Whether `path` (POSIX-separated, relative to the root) is ignored itself or lies under
    /// an ignored directory.
    #[must_use]
    pub fn covers(&self, path: &str) -> bool {
        let entries = self.0.read().unwrap_or_else(PoisonError::into_inner);
        if entries.contains(path) || entries.contains(&format!("{path}{DIRECTORY_MARK}")) {
            return true;
        }
        path.match_indices(DIRECTORY_MARK)
            .any(|(at, _)| entries.contains(&path[..=at]))
    }
}

#[cfg(test)]
#[path = "ignored_paths_tests.rs"]
mod tests;
