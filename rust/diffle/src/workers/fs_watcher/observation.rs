//! Reads one OS event: does it matter, and may it have created directories to watch?

use std::path::PathBuf;

use notify::event::ModifyKind;
use notify::{Event, EventKind};

use super::scope::event_filter::{EventFilter, Verdict};

/// A change that makes the review stale.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Observation {
    /// Checkout paths that just appeared (created or renamed in), any of which may be a
    /// directory that now needs its own watch.
    pub appeared: Vec<PathBuf>,
}

/// `None` for events that change nothing: reads and opens (Git itself reads the checkout
/// on every refresh, which would loop), and paths the filter ignores.
#[must_use]
pub fn observe(event: &Event, filter: &EventFilter) -> Option<Observation> {
    // The kernel dropped events: anything may have changed.
    if event.need_rescan() {
        return Some(Observation::default());
    }
    let mut mattered = false;
    let mut appeared = Vec::new();
    for path in &event.paths {
        match (is_read_only(&event.kind), filter.verdict(path)) {
            (true, _) | (_, Verdict::Ignored) => {}
            (false, Verdict::Metadata) => mattered = true,
            (false, Verdict::Checkout) => {
                mattered = true;
                if introduces_paths(&event.kind) {
                    appeared.push(path.clone());
                }
            }
        }
    }
    mattered.then_some(Observation { appeared })
}

fn is_read_only(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(_) => true,
        EventKind::Any
        | EventKind::Create(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => false,
    }
}

fn introduces_paths(kind: &EventKind) -> bool {
    match kind {
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Any
        | EventKind::Access(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use notify::event::{AccessKind, AccessMode, CreateKind, DataChange, RemoveKind, RenameMode};

    use super::*;
    use crate::workers::fs_watcher::scope::watch_target::WatchTarget;

    fn filter() -> EventFilter {
        EventFilter::new(&WatchTarget::Worktree {
            root: "/wt".into(),
            git_dir: "/wt/.git".into(),
            common_dir: "/wt/.git".into(),
            ignored: Arc::new(|rel| rel.starts_with("target")),
        })
    }

    fn event(kind: EventKind, paths: &[&str]) -> Event {
        paths.iter().fold(Event::new(kind), |event, path| {
            event.add_path((*path).into())
        })
    }

    #[test]
    fn reads_never_matter() {
        let read = EventKind::Access(AccessKind::Close(AccessMode::Read));
        assert_eq!(
            observe(&event(read, &["/wt/a.rs", "/wt/.git/index"]), &filter()),
            None
        );
    }

    #[test]
    fn ignored_paths_never_matter() {
        let write = EventKind::Modify(ModifyKind::Data(DataChange::Any));
        assert_eq!(
            observe(
                &event(write, &["/wt/target/x", "/wt/.git/objects/a"]),
                &filter()
            ),
            None
        );
    }

    #[test]
    fn a_write_matters_without_appearing() {
        let write = EventKind::Modify(ModifyKind::Data(DataChange::Any));
        let seen = observe(&event(write, &["/wt/a.rs"]), &filter());
        assert_eq!(seen, Some(Observation::default()));
    }

    #[test]
    fn a_creation_lists_the_new_checkout_path() {
        let create = EventKind::Create(CreateKind::Folder);
        let seen = observe(&event(create, &["/wt/new"]), &filter()).unwrap();
        assert_eq!(seen.appeared, [PathBuf::from("/wt/new")]);
    }

    #[test]
    fn a_rename_matters_if_either_end_does_and_lists_only_checkout_ends() {
        let rename = EventKind::Modify(ModifyKind::Name(RenameMode::Both));
        let seen = observe(
            &event(rename, &["/wt/.git/index.lock", "/wt/.git/index"]),
            &filter(),
        );
        assert_eq!(seen, Some(Observation::default()));
        let moved_in = observe(&event(rename, &["/wt/target/a", "/wt/dir"]), &filter()).unwrap();
        assert_eq!(moved_in.appeared, [PathBuf::from("/wt/dir")]);
    }

    #[test]
    fn a_removal_matters_without_appearing() {
        let remove = EventKind::Remove(RemoveKind::File);
        assert_eq!(
            observe(&event(remove, &["/wt/a.rs"]), &filter()),
            Some(Observation::default())
        );
    }

    #[test]
    fn lost_events_matter_even_without_a_path() {
        let lost = Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert_eq!(observe(&lost, &filter()), Some(Observation::default()));
    }

    #[test]
    fn a_pathless_event_matters_not() {
        assert_eq!(observe(&Event::new(EventKind::Any), &filter()), None);
    }
}
