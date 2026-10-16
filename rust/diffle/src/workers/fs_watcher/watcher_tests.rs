use std::fs;
use std::sync::Arc;

use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::workers::fs_watcher::test_support::{
    DEBOUNCE, Listed, QUIET_FOR, Repo, expect_quiet, expect_signal, settle, watch_refs_result,
    watch_worktree,
};

#[tokio::test]
async fn a_checkout_edit_signals() {
    let repo = Repo::init();
    fs::write(repo.root().join("a.txt"), "1").unwrap();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    fs::write(repo.root().join("a.txt"), "2").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_burst_of_edits_yields_one_signal() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    for n in 0..25 {
        fs::write(repo.root().join(format!("f{n}.txt")), "x").unwrap();
    }
    expect_signal(&mut signals).await;
    expect_quiet(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn reading_a_file_is_not_a_change() {
    let repo = Repo::init();
    fs::write(repo.root().join("a.txt"), "1").unwrap();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    fs::read(repo.root().join("a.txt")).unwrap();
    expect_quiet(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_lock_file_in_the_checkout_is_a_change() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    fs::write(repo.root().join("Cargo.lock"), "").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn ignored_files_and_directories_stay_silent_and_the_rule_can_change() {
    let repo = Repo::init();
    fs::create_dir(repo.root().join("target")).unwrap();
    let listed = Listed::default();
    listed
        .write()
        .unwrap()
        .extend(["debug.log".to_owned(), "target".to_owned()]);
    let (watcher, mut signals) = watch_worktree(&repo, &listed).await;

    fs::write(repo.root().join("debug.log"), "x").unwrap();
    fs::write(repo.root().join("target/out.o"), "x").unwrap();
    expect_quiet(&mut signals).await;

    listed.write().unwrap().remove("debug.log");
    fs::write(repo.root().join("debug.log"), "y").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_directory_created_later_is_watched_unless_ignored() {
    let repo = Repo::init();
    let listed = Listed::default();
    listed.write().unwrap().insert("build".to_owned());
    let (watcher, mut signals) = watch_worktree(&repo, &listed).await;

    fs::create_dir(repo.root().join("build")).unwrap();
    fs::write(repo.root().join("build/out.o"), "x").unwrap();
    expect_quiet(&mut signals).await;

    fs::create_dir_all(repo.root().join("src/deep")).unwrap();
    settle(&mut signals).await;
    fs::write(repo.root().join("src/deep/lib.rs"), "x").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_directory_removed_while_watched_does_not_stop_the_watcher() {
    let repo = Repo::init();
    fs::create_dir_all(repo.root().join("sub/inner")).unwrap();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    fs::remove_dir_all(repo.root().join("sub")).unwrap();
    settle(&mut signals).await;
    fs::write(repo.root().join("after.txt"), "x").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn a_lagging_consumer_holds_one_pending_signal_and_never_blocks_the_watcher() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    for n in 0..4 {
        fs::write(repo.root().join(format!("f{n}.txt")), "x").unwrap();
        tokio::time::sleep(DEBOUNCE * 4).await;
    }
    assert_eq!(signals.try_recv(), Ok(()));
    assert_eq!(signals.try_recv(), Err(TryRecvError::Empty));
    fs::write(repo.root().join("late.txt"), "x").unwrap();
    expect_signal(&mut signals).await;
    watcher.close().await;
}

#[tokio::test]
async fn closing_ends_the_signals() {
    let repo = Repo::init();
    let (watcher, mut signals) = watch_worktree(&repo, &Listed::default()).await;
    watcher.close().await;
    fs::write(repo.root().join("a.txt"), "x").unwrap();
    let ended = tokio::time::timeout(QUIET_FOR, signals.recv()).await;
    assert_eq!(ended.unwrap(), None);
}

#[tokio::test]
async fn a_pending_signal_is_cancelled_by_closing() {
    let repo = Repo::init();
    let target = WatchTarget::Worktree {
        root: repo.root().into(),
        git_dir: repo.git_dir(),
        common_dir: repo.git_dir(),
        ignored: Arc::new(|_| false),
    };
    let (watcher, mut signals) = FsWatcher::start(target, QUIET_FOR).await.unwrap();
    fs::write(repo.root().join("a.txt"), "x").unwrap();
    tokio::time::sleep(DEBOUNCE).await;
    watcher.close().await;
    assert_eq!(signals.recv().await, None);
}

#[tokio::test]
async fn a_missing_checkout_fails_to_start_naming_it() {
    let repo = Repo::init();
    let missing = repo.root().join("missing");
    let target = WatchTarget::Worktree {
        root: missing.clone(),
        git_dir: repo.git_dir(),
        common_dir: repo.git_dir(),
        ignored: Arc::new(|_| false),
    };
    let error = FsWatcher::start(target, DEBOUNCE).await.err().unwrap();
    assert!(
        matches!(&error, WatchError::Watch { path, .. } if *path == missing),
        "{error}"
    );
}

#[tokio::test]
async fn missing_git_metadata_fails_to_start() {
    let repo = Repo::init();
    fs::remove_dir_all(repo.git_dir().join("refs")).unwrap();
    let error = watch_refs_result(&repo).await.err().unwrap();
    assert!(matches!(error, WatchError::Watch { .. }), "{error}");

    let nowhere = repo.root().join("nowhere");
    let target = WatchTarget::Refs {
        git_dir: nowhere.clone(),
        common_dir: nowhere,
    };
    assert!(FsWatcher::start(target, DEBOUNCE).await.is_err());
}
