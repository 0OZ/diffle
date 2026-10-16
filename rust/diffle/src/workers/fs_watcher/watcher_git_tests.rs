//! Git's own ways of moving metadata, as the watcher must see them.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::workers::fs_watcher::test_support::{
    DEBOUNCE, Listed, Repo, expect_quiet, expect_signal, replace_atomically, run_git, settle,
    watch_refs, watch_worktree,
};

#[tokio::test]
async fn git_internals_and_lock_files_stay_silent() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    let git_dir = repo.git_dir();
    fs::write(git_dir.join("index.lock"), "").unwrap();
    fs::write(git_dir.join("HEAD.lock"), "").unwrap();
    fs::write(git_dir.join("refs/heads/topic.lock"), "").unwrap();
    fs::write(git_dir.join("COMMIT_EDITMSG"), "x").unwrap();
    fs::write(git_dir.join("objects/junk"), "x").unwrap();
    expect_quiet(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn atomically_replacing_head_signals_every_time() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    for branch in ["one", "two", "three"] {
        replace_atomically(
            &repo.git_dir().join("HEAD"),
            format!("ref: refs/heads/{branch}\n"),
        );
        expect_signal(&mut signals).await;
    }
    watcher.close().await;
}

#[tokio::test]
async fn atomically_replacing_packed_refs_signals_every_time() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    for round in 0..3 {
        replace_atomically(
            &repo.git_dir().join("packed-refs"),
            format!("# round {round}\n"),
        );
        expect_signal(&mut signals).await;
    }
    watcher.close().await;
}

#[tokio::test]
async fn atomically_replacing_the_index_signals_every_time() {
    let repo = Repo::init();
    repo.commit_file("a.txt", "1");
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    let index = repo.git_dir().join("index");
    let bytes = fs::read(&index).unwrap();
    for _ in 0..3 {
        replace_atomically(&index, &bytes);
        expect_signal(&mut signals).await;
    }
    watcher.close().await;
}

#[tokio::test]
async fn a_new_ref_signals_including_in_a_new_ref_directory() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    let heads = repo.git_dir().join("refs/heads");
    fs::write(heads.join("feature"), "0\n").unwrap();
    settle(&mut signals).await;
    fs::create_dir(heads.join("team")).unwrap();
    settle(&mut signals).await;
    fs::write(heads.join("team/topic"), "0\n").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_commit_signals_once_the_lock_files_notwithstanding() {
    let repo = Repo::init();
    repo.commit_file("a.txt", "1");
    let target = WatchTarget::Worktree {
        root: repo.root().into(),
        git_dir: repo.git_dir(),
        common_dir: repo.git_dir(),
        ignored: Arc::new(|_| false),
    };
    let (watcher, mut signals) = FsWatcher::start(target, DEFAULT_DEBOUNCE).await.unwrap();
    repo.git(&["commit", "--quiet", "--allow-empty", "-m", "empty"]);
    expect_signal(&mut signals).await;
    expect_quiet(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_refs_target_follows_refs_and_ignores_the_checkout_and_index() {
    let repo = Repo::init();
    repo.commit_file("a.txt", "1");
    let (watcher, mut signals) = watch_refs(&repo).await;
    fs::write(repo.root().join("a.txt"), "2").unwrap();
    let index = repo.git_dir().join("index");
    replace_atomically(&index, fs::read(&index).unwrap());
    expect_quiet(&mut signals).await;

    replace_atomically(&repo.git_dir().join("HEAD"), "ref: refs/heads/other\n");
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_linked_worktree_follows_its_own_git_dir_and_the_shared_refs() {
    let repo = Repo::init();
    repo.commit_file("a.txt", "1");
    let holder = tempfile::tempdir().unwrap();
    let linked = holder.path().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "--quiet",
        linked.to_str().unwrap(),
        "-b",
        "side",
    ]);
    let git_dir = run_git(&linked, &["rev-parse", "--absolute-git-dir"]);
    let common = run_git(
        &linked,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    assert_ne!(git_dir, common);
    let target = WatchTarget::Worktree {
        root: linked.clone(),
        git_dir: git_dir.clone().into(),
        common_dir: common.clone().into(),
        ignored: Arc::new(|_| false),
    };
    let (watcher, mut signals) = FsWatcher::start(target, DEBOUNCE).await.unwrap();

    let index = Path::new(&git_dir).join("index");
    replace_atomically(&index, fs::read(&index).unwrap());
    settle(&mut signals).await;
    fs::write(Path::new(&common).join("refs/heads/shared"), "0\n").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}
