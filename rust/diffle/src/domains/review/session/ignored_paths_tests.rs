use super::*;

fn ignoring(entries: &[&str]) -> IgnoredPaths {
    let ignored = IgnoredPaths::default();
    ignored.replace(entries.iter().map(|entry| (*entry).to_owned()));
    ignored
}

#[test]
fn covers_a_file_a_directory_and_everything_below_it() {
    let ignored = ignoring(&["debug.log", "target/", "a/b/"]);
    assert!(ignored.covers("debug.log"));
    assert!(ignored.covers("target"));
    assert!(ignored.covers("target/debug/app"));
    assert!(ignored.covers("a/b/c/d.txt"));
    assert!(!ignored.covers("a/c.txt"));
    assert!(!ignored.covers("src/debug.log"));
    assert!(!ignored.covers("targets/x"));
}

#[test]
fn nothing_is_ignored_until_told() {
    assert!(!IgnoredPaths::default().covers("anything"));
}

#[test]
fn clones_share_replacements() {
    let ignored = ignoring(&["old/"]);
    let watcher_side = ignored.clone();
    assert!(watcher_side.covers("old/x"));
    ignored.replace(["new/".to_owned()]);
    assert!(!watcher_side.covers("old/x"));
    assert!(watcher_side.covers("new/x"));
}
