use std::sync::Arc;

use super::*;
use crate::workers::fs_watcher::scope::watch_target::WatchTarget;

fn mkdirs(root: &Path, dirs: &[&str]) {
    for dir in dirs {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
}

fn walk(root: &Path, git_dir: &Path, ignored: &'static [&'static str]) -> Vec<String> {
    let filter = EventFilter::new(&WatchTarget::Worktree {
        root: root.into(),
        git_dir: git_dir.into(),
        common_dir: git_dir.into(),
        ignored: Arc::new(move |rel| {
            rel.split('/')
                .scan(String::new(), |prefix, part| {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(part);
                    Some(prefix.clone())
                })
                .any(|prefix| ignored.contains(&prefix.as_str()))
        }),
    });
    let mut dirs: Vec<String> = watchable_dirs(root, &filter)
        .iter()
        .map(|dir| {
            dir.strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn collects_nested_directories_and_prunes_git_and_ignored_trees() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    mkdirs(
        root,
        &[
            "src/deep",
            ".git/objects/ab",
            "node_modules/pkg/lib",
            "web/dist",
        ],
    );
    fs::write(root.join("src/file.rs"), "").unwrap();
    let dirs = walk(root, &root.join(".git"), &["node_modules", "web/dist"]);
    assert_eq!(dirs, ["", "src", "src/deep", "web"]);
}

#[test]
fn prunes_a_git_dir_outside_the_root_by_never_meeting_it() {
    let tmp = tempfile::tempdir().unwrap();
    mkdirs(tmp.path(), &["wt/a", "gitdir/objects"]);
    let dirs = walk(&tmp.path().join("wt"), &tmp.path().join("gitdir"), &[]);
    assert_eq!(dirs, ["", "a"]);
}

#[cfg(unix)]
#[test]
fn does_not_follow_symlinked_directories() {
    let tmp = tempfile::tempdir().unwrap();
    mkdirs(tmp.path(), &["wt/real", "outside/secret"]);
    std::os::unix::fs::symlink(tmp.path().join("outside"), tmp.path().join("wt/link")).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("wt"), tmp.path().join("wt/real/loop")).unwrap();
    assert_eq!(
        walk(&tmp.path().join("wt"), &tmp.path().join("g"), &[]),
        ["", "real"]
    );
}

#[test]
fn a_missing_or_file_start_yields_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("file"), "").unwrap();
    assert!(walk(&tmp.path().join("missing"), &tmp.path().join("g"), &[]).is_empty());
    let filter = EventFilter::new(&WatchTarget::Worktree {
        root: tmp.path().into(),
        git_dir: tmp.path().join("g"),
        common_dir: tmp.path().join("g"),
        ignored: Arc::new(|_| false),
    });
    assert!(watchable_dirs(&tmp.path().join("file"), &filter).is_empty());
}
