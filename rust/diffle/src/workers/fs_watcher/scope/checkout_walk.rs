//! Which directories of a checkout deserve an OS watch.
//!
//! A recursive OS watch would also cover `node_modules` and `target`, which can exhaust the
//! system's watches; walking by hand lets ignored trees go unwatched.

use std::fs;
use std::path::{Path, PathBuf};

use super::event_filter::{EventFilter, Verdict};

/// `start` and every real directory beneath it that the filter counts as checkout.
/// Symlinked directories are not followed (no cycles, nothing outside the checkout);
/// unreadable or vanished ones are skipped. Empty when `start` is not such a directory.
#[must_use]
pub fn watchable_dirs(start: &Path, filter: &EventFilter) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![start.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let is_real_dir = fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir());
        if !is_real_dir || filter.verdict(&dir) != Verdict::Checkout {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
        found.push(dir);
    }
    found
}

#[cfg(test)]
#[path = "checkout_walk_tests.rs"]
mod tests;
