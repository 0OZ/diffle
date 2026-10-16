use std::fs as stdfs;

use serde_json::json;

use super::*;
use crate::domains::comments::comment_error::NotFound;
use crate::domains::comments::storage::scratch_store::{
    all_threads, anchor, body, comments_json, entry, first_bodies, first_bodies_of, open,
};

#[tokio::test]
async fn two_stores_on_one_file_keep_each_others_sets() {
    let dir = tempfile::tempdir().unwrap();
    let working = open(&dir, "working").await;
    let pr = open(&dir, "pr:abc").await;
    working.add_thread(anchor(), body("w")).await.unwrap();
    pr.add_thread(anchor(), body("p")).await.unwrap();
    working
        .set_viewed(entry("a.py", "sha", true))
        .await
        .unwrap();
    let raw = comments_json(&dir);
    assert_eq!(first_bodies(&raw, "working"), ["w"]);
    assert_eq!(first_bodies(&raw, "pr:abc"), ["p"]);
    assert_eq!(all_threads(&open(&dir, "pr:abc").await).await.len(), 1);
}

#[tokio::test]
async fn another_keys_set_is_kept_verbatim_even_with_fields_this_version_does_not_know() {
    let dir = tempfile::tempdir().unwrap();
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    let foreign = json!({"threads": "not even threads", "extra": [1, 2]});
    let raw = json!({"version": 3, "sets": {"other": foreign}});
    stdfs::write(dir.path().join("diffle/comments.json"), raw.to_string()).unwrap();
    open(&dir, "working")
        .await
        .add_thread(anchor(), body("w"))
        .await
        .unwrap();
    assert_eq!(comments_json(&dir)["sets"]["other"], foreign);
}

#[tokio::test]
async fn two_open_stores_on_one_key_see_and_keep_each_others_writes() {
    let dir = tempfile::tempdir().unwrap();
    let a = open(&dir, "working").await;
    let b = open(&dir, "working").await;
    let thread = a.add_thread(anchor(), body("from a")).await.unwrap();
    // b never loaded the thread, yet serves it and must not drop it on its own write.
    assert_eq!(all_threads(&b).await, std::slice::from_ref(&thread));
    b.set_viewed(entry("a.py", "sha", true)).await.unwrap();
    b.reply(thread.id(), body("from b")).await.unwrap();
    let bodies: Vec<String> = a
        .get(thread.id())
        .await
        .unwrap()
        .unwrap()
        .messages()
        .iter()
        .map(|m| m.text().as_str().to_owned())
        .collect();
    assert_eq!(bodies, ["from a", "from b"]);
    assert_eq!(a.viewed().await.unwrap(), [entry("a.py", "sha", true)]);
    let fresh = open(&dir, "working").await;
    assert_eq!(all_threads(&fresh).await.len(), 1);
    assert_eq!(fresh.viewed().await.unwrap().len(), 1);
}

#[tokio::test]
async fn concurrent_writes_from_several_stores_and_keys_all_survive_reload() {
    // An old-mode store finishing a write while the new mode's store starts, plus a second server
    // on the same key.
    let dir = tempfile::tempdir().unwrap();
    let old = open(&dir, "working").await;
    let next = open(&dir, "pr:abc").await;
    let other = open(&dir, "working").await;
    let (a, b, c, d, e, f) = tokio::join!(
        old.set_viewed(entry("a.py", "sha", true)),
        next.add_thread(anchor(), body("n1")),
        old.add_thread(anchor(), body("o1")),
        next.set_viewed(entry("b.py", "sha", true)),
        other.add_thread(anchor(), body("o2")),
        next.add_thread(anchor(), body("n2")),
    );
    a.unwrap();
    b.unwrap();
    c.unwrap();
    d.unwrap();
    e.unwrap();
    f.unwrap();
    let working = open(&dir, "working").await;
    let pr = open(&dir, "pr:abc").await;
    assert_eq!(first_bodies_of(&working).await, ["o1", "o2"]);
    assert_eq!(
        working.viewed().await.unwrap(),
        [entry("a.py", "sha", true)]
    );
    assert_eq!(first_bodies_of(&pr).await, ["n1", "n2"]);
    assert_eq!(pr.viewed().await.unwrap(), [entry("b.py", "sha", true)]);
    // Unique temp names and a released lock: nothing left beside the file.
    let mut names: Vec<_> = stdfs::read_dir(dir.path().join("diffle"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["comments.json"]);
}

#[tokio::test]
async fn a_deletion_by_one_store_is_not_resurrected_by_another() {
    let dir = tempfile::tempdir().unwrap();
    let a = open(&dir, "working").await;
    let b = open(&dir, "working").await;
    let id = a
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    assert!(b.get(&id).await.unwrap().is_some());
    a.remove_thread(&id).await.unwrap();
    b.set_viewed(entry("a.py", "sha", true)).await.unwrap();
    let late = b.reply(&id, body("late")).await.unwrap_err();
    assert!(
        matches!(late, CommentError::NotFound(NotFound::Thread(_))),
        "{late}"
    );
    assert!(all_threads(&a).await.is_empty());
    assert!(all_threads(&open(&dir, "working").await).await.is_empty());
}

#[tokio::test]
async fn a_failed_write_is_not_acknowledged_and_does_not_block_the_next_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    // A directory where the file belongs makes the read-modify-write fail.
    stdfs::create_dir_all(store.file()).unwrap();
    let failed = store.add_thread(anchor(), body("first")).await.unwrap_err();
    assert!(matches!(failed, CommentError::Io { .. }), "{failed}");
    stdfs::remove_dir(store.file()).unwrap();
    store.add_thread(anchor(), body("second")).await.unwrap();
    assert_eq!(first_bodies(&comments_json(&dir), "working"), ["second"]);
    let names: Vec<_> = stdfs::read_dir(dir.path().join("diffle"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["comments.json"]);
}

#[tokio::test]
async fn waits_for_a_live_lock_and_takes_over_a_stale_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let lock = dir.path().join("diffle/comments.json.lock");
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    // Held by this (live) process: the write waits until the lock goes away.
    stdfs::write(&lock, format!("{}\n", std::process::id())).unwrap();
    let writer = store.clone();
    let write = tokio::spawn(async move { writer.add_thread(anchor(), body("hello")).await });
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert!(!write.is_finished());
    stdfs::remove_file(&lock).unwrap();
    write.await.unwrap().unwrap();
    assert_eq!(all_threads(&store).await.len(), 1);
    // Left behind by a dead process: taken over at once.
    stdfs::write(&lock, "999999999\n").unwrap();
    store.add_thread(anchor(), body("hello")).await.unwrap();
    assert_eq!(all_threads(&store).await.len(), 2);
    assert!(!lock.exists());
}
