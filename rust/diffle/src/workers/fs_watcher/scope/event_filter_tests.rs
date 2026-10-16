use std::sync::{Arc, RwLock};

use super::*;
use crate::workers::fs_watcher::test_support::{Listed, rule_over};

fn worktree(listed: &Listed) -> EventFilter {
    EventFilter::new(&WatchTarget::Worktree {
        root: "/wt".into(),
        git_dir: "/wt/.git".into(),
        common_dir: "/wt/.git".into(),
        ignored: rule_over(listed),
    })
}

fn verdict(filter: &EventFilter, path: &str) -> Verdict {
    filter.verdict(Path::new(path))
}

#[test]
fn checkout_paths_count_unless_ignored() {
    let listed = Listed::default();
    listed
        .write()
        .unwrap()
        .extend(["debug.log".into(), "target".into()]);
    let filter = worktree(&listed);
    assert_eq!(verdict(&filter, "/wt/src/main.rs"), Verdict::Checkout);
    assert_eq!(verdict(&filter, "/wt"), Verdict::Checkout);
    assert_eq!(verdict(&filter, "/wt/debug.log"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/wt/target/debug/app"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/wt/targets/a"), Verdict::Checkout);
}

#[test]
fn the_rule_is_consulted_per_event() {
    let listed = Listed::default();
    let filter = worktree(&listed);
    assert_eq!(verdict(&filter, "/wt/a.txt"), Verdict::Checkout);
    listed.write().unwrap().insert("a.txt".into());
    assert_eq!(verdict(&filter, "/wt/a.txt"), Verdict::Ignored);
}

#[test]
fn the_rule_receives_a_posix_path_relative_to_the_root() {
    let seen: Arc<RwLock<Vec<String>>> = Arc::default();
    let sink = Arc::clone(&seen);
    let filter = EventFilter::new(&WatchTarget::Worktree {
        root: "/wt".into(),
        git_dir: "/wt/.git".into(),
        common_dir: "/wt/.git".into(),
        ignored: Arc::new(move |rel| {
            sink.write().unwrap().push(rel.to_owned());
            false
        }),
    });
    verdict(&filter, "/wt/a/b/c.txt");
    verdict(&filter, "/wt");
    assert_eq!(*seen.read().unwrap(), ["a/b/c.txt"]);
}

#[test]
fn the_git_dir_inside_the_checkout_yields_metadata_only() {
    let filter = worktree(&Listed::default());
    assert_eq!(verdict(&filter, "/wt/.git/HEAD"), Verdict::Metadata);
    assert_eq!(verdict(&filter, "/wt/.git/index"), Verdict::Metadata);
    assert_eq!(verdict(&filter, "/wt/.git/refs/heads/x"), Verdict::Metadata);
    assert_eq!(verdict(&filter, "/wt/.git/objects/ab/cd"), Verdict::Ignored);
    assert_eq!(
        verdict(&filter, "/wt/.git/COMMIT_EDITMSG"),
        Verdict::Ignored
    );
}

#[test]
fn a_dot_git_of_a_nested_checkout_path_is_ignored_even_outside_the_git_dir() {
    let filter = EventFilter::new(&WatchTarget::Worktree {
        root: "/wt".into(),
        git_dir: "/elsewhere/gd".into(),
        common_dir: "/elsewhere/gd".into(),
        ignored: Arc::new(|_| false),
    });
    assert_eq!(verdict(&filter, "/wt/.git"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/wt/.git/config"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/elsewhere/gd/HEAD"), Verdict::Metadata);
}

#[test]
fn lock_files_in_git_are_ignored_and_in_the_checkout_are_not() {
    let filter = worktree(&Listed::default());
    assert_eq!(verdict(&filter, "/wt/.git/index.lock"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/wt/.git/HEAD.lock"), Verdict::Ignored);
    assert_eq!(
        verdict(&filter, "/wt/.git/refs/heads/x.lock"),
        Verdict::Ignored
    );
    assert_eq!(verdict(&filter, "/wt/Cargo.lock"), Verdict::Checkout);
    assert_eq!(verdict(&filter, "/wt/sub/.lock"), Verdict::Checkout);
}

#[test]
fn a_refs_target_sees_no_checkout() {
    let filter = EventFilter::new(&WatchTarget::Refs {
        git_dir: "/r/.git".into(),
        common_dir: "/r/.git".into(),
    });
    assert_eq!(verdict(&filter, "/r/src/main.rs"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/r/.git/index"), Verdict::Ignored);
    assert_eq!(verdict(&filter, "/r/.git/HEAD"), Verdict::Metadata);
    assert!(filter.checkout_root().is_none());
}
