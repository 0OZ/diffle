//! The verdict on one path: does a change there make the review stale?

use std::path::{Component, Path, PathBuf};

use super::metadata_paths::MetadataPaths;
use super::watch_target::{IgnoreRule, WatchTarget};

/// Git's own bookkeeping directory name inside a checkout.
const DOT_GIT: &str = ".git";
/// Git writes `<name>.lock` next to a file and renames it over; the rename is the change.
const LOCK_SUFFIX: &str = ".lock";
const POSIX_SEPARATOR: &str = "/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ignored,
    /// A Git metadata file or ref moved.
    Metadata,
    /// A path in the checkout that Git does not ignore.
    Checkout,
}

pub struct EventFilter {
    metadata: MetadataPaths,
    checkout: Option<Checkout>,
}

struct Checkout {
    root: PathBuf,
    ignored: IgnoreRule,
}

impl EventFilter {
    #[must_use]
    pub fn new(target: &WatchTarget) -> Self {
        Self {
            metadata: MetadataPaths::of(target),
            checkout: target.checkout().map(|(root, ignored)| Checkout {
                root: root.to_path_buf(),
                ignored: ignored.clone(),
            }),
        }
    }

    #[must_use]
    pub fn metadata(&self) -> &MetadataPaths {
        &self.metadata
    }

    #[must_use]
    pub fn checkout_root(&self) -> Option<&Path> {
        self.checkout
            .as_ref()
            .map(|checkout| checkout.root.as_path())
    }

    /// Inside a Git directory only metadata counts, and never a lock file, so a commit
    /// yields one refresh. A `.lock` in the checkout is an ordinary file.
    #[must_use]
    pub fn verdict(&self, path: &Path) -> Verdict {
        if self.metadata.is_inside_git(path) {
            return if !is_lock(path) && self.metadata.is_metadata(path) {
                Verdict::Metadata
            } else {
                Verdict::Ignored
            };
        }
        let Some(checkout) = &self.checkout else {
            return Verdict::Ignored;
        };
        let Ok(rel) = path.strip_prefix(&checkout.root) else {
            return Verdict::Ignored;
        };
        let segments: Vec<_> = rel
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_string_lossy()),
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => None,
            })
            .collect();
        if segments.first().is_some_and(|first| first == DOT_GIT)
            || (!segments.is_empty() && (checkout.ignored)(&segments.join(POSIX_SEPARATOR)))
        {
            return Verdict::Ignored;
        }
        Verdict::Checkout
    }
}

fn is_lock(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.as_encoded_bytes().ends_with(LOCK_SUFFIX.as_bytes()))
}

#[cfg(test)]
#[path = "event_filter_tests.rs"]
mod tests;
