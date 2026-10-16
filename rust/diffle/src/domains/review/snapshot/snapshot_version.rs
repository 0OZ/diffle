//! Which generation of the review a snapshot belongs to.

use std::fmt;

/// Strictly increasing over a run; only the session's transition queue advances it, so two
/// snapshots never share one and a client can tell a stale answer by comparing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SnapshotVersion(u64);

impl SnapshotVersion {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The version after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for SnapshotVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_only_go_up() {
        let first = SnapshotVersion::default().next();
        assert_eq!(first.get(), 1);
        assert!(first.next() > first);
        assert_eq!(first.next().to_string(), "2");
    }
}
