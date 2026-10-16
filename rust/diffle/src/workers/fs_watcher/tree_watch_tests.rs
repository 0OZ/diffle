use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::*;
use crate::workers::fs_watcher::scope::watch_target::WatchTarget;

fn filter_for(root: &Path) -> EventFilter {
    EventFilter::new(&WatchTarget::Worktree {
        root: root.into(),
        git_dir: root.join(".git"),
        common_dir: root.join(".git"),
        ignored: Arc::new(|rel| rel.starts_with("skipped")),
    })
}

/// A watcher whose only job is to flag that it heard something.
fn listening(heard: &Arc<AtomicBool>) -> notify::RecommendedWatcher {
    let heard = Arc::clone(heard);
    notify::recommended_watcher(move |_event: notify::Result<notify::Event>| {
        heard.store(true, Ordering::SeqCst);
    })
    .unwrap()
}

#[test]
fn a_required_missing_path_is_an_error_naming_it() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("missing");
    let mut watcher = listening(&Arc::default());
    let error = watch_required(&mut watcher, &missing, RecursiveMode::NonRecursive).unwrap_err();
    assert!(
        matches!(&error, WatchError::Watch { path, .. } if *path == missing),
        "{error}"
    );
}

#[test]
fn the_tree_watch_hears_kept_directories_and_not_pruned_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for dir in ["kept/deep", "skipped/deep"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    let heard = Arc::new(AtomicBool::new(false));
    let mut watcher = listening(&heard);
    watch_checkout_tree(&mut watcher, root, &filter_for(root)).unwrap();

    std::fs::write(root.join("skipped/deep/a"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(!heard.load(Ordering::SeqCst));

    std::fs::write(root.join("kept/deep/a"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(heard.load(Ordering::SeqCst));
}

#[test]
fn a_vanished_start_is_not_a_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let mut watcher = listening(&Arc::default());
    let gone = tmp.path().join("gone");
    assert!(watch_checkout_tree(&mut watcher, &gone, &filter_for(tmp.path())).is_ok());
}
