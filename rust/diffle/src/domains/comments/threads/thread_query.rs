//! Which threads a read asks for.

use super::thread::CommentThread;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThreadState {
    Open,
    Resolved,
    #[default]
    All,
}

/// A filter over a review's threads: by state, and by the path they are filed under.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadQuery {
    state: ThreadState,
    path: Option<String>,
}

impl ThreadQuery {
    #[must_use]
    pub fn new(state: ThreadState, path: Option<String>) -> Self {
        Self { state, path }
    }

    /// Every thread.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn admits(&self, thread: &CommentThread) -> bool {
        let state_ok = match (self.state, thread.is_resolved()) {
            (ThreadState::All, _) | (ThreadState::Open, false) | (ThreadState::Resolved, true) => {
                true
            }
            (ThreadState::Open, true) | (ThreadState::Resolved, false) => false,
        };
        state_ok
            && self
                .path
                .as_deref()
                .is_none_or(|path| thread.anchor().path().as_str() == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::anchors::anchor::Anchor;
    use crate::domains::comments::sample_threads::{path, thread};

    #[test]
    fn a_query_filters_by_state_and_path() {
        let open = thread("o", Anchor::File(path("a.py")), "x", 1);
        let mut resolved = thread("r", Anchor::File(path("z.py")), "x", 1);
        resolved.set_resolved(true, crate::kernel::epoch_millis::EpochMillis::new(2));
        let admits = |q: &ThreadQuery| (q.admits(&open), q.admits(&resolved));
        assert_eq!(admits(&ThreadQuery::all()), (true, true));
        assert_eq!(
            admits(&ThreadQuery::new(ThreadState::Open, None)),
            (true, false)
        );
        assert_eq!(
            admits(&ThreadQuery::new(ThreadState::Resolved, None)),
            (false, true)
        );
        assert_eq!(
            admits(&ThreadQuery::new(ThreadState::All, Some("z.py".into()))),
            (false, true)
        );
    }
}
