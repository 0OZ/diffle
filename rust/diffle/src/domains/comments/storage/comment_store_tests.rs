use std::fs as stdfs;

use super::*;
use crate::domains::comments::comment_error::NotFound;
use crate::domains::comments::sample_threads::line_anchor;
use crate::domains::comments::storage::scratch_store::{
    all_threads, anchor, body, comments_json, entry, open,
};
use crate::domains::comments::threads::thread::Resolution;
use crate::domains::comments::threads::thread_query::ThreadState;
use crate::kernel::side::Side;

#[tokio::test]
async fn persists_threads_per_key_and_reloads_them() {
    let dir = tempfile::tempdir().unwrap();
    let first = open(&dir, "working").await;
    let thread = first.add_thread(anchor(), body("hello")).await.unwrap();
    assert!(!thread.id().as_str().is_empty());
    assert_eq!(thread.messages().first().text().as_str(), "hello");
    let second = open(&dir, "working").await;
    assert_eq!(all_threads(&second).await, [thread]);
    let other = open(&dir, "pr:abc").await;
    assert!(all_threads(&other).await.is_empty());
    let raw = comments_json(&dir);
    assert_eq!(raw["version"], 3);
    let keys: Vec<&String> = raw["sets"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["working"]);
}

// Quoted source lines are owner-only.
#[cfg(unix)]
#[tokio::test]
async fn writes_the_comment_file_and_its_directory_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    store.add_thread(anchor(), body("hello")).await.unwrap();
    let mode = |p: &Path| stdfs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(store.file()), 0o600);
    assert_eq!(mode(&dir.path().join("diffle")), 0o700);
}

#[tokio::test]
async fn replies_edits_removes_messages_and_threads_resolves_and_clears() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    store.reply(&id, body("fixed")).await.unwrap();
    let texts = |thread: &CommentThread| -> Vec<String> {
        thread
            .messages()
            .iter()
            .map(|m| m.text().as_str().to_owned())
            .collect()
    };
    let thread = store.get(&id).await.unwrap().unwrap();
    assert_eq!(texts(&thread), ["hello", "fixed"]);
    let first = thread.messages().first().id().clone();
    let second = thread.messages().iter().nth(1).unwrap().id().clone();
    store
        .edit_message(&id, &first, MessageText::new("hello again"))
        .await
        .unwrap();
    assert_eq!(
        texts(&store.get(&id).await.unwrap().unwrap())[0],
        "hello again"
    );
    let kept = store.remove_message(&id, &second).await.unwrap();
    assert_eq!(kept.unwrap().id(), &id);
    assert_eq!(
        store
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .messages()
            .iter()
            .count(),
        1
    );

    store.set_resolved(&id, true).await.unwrap();
    let resolved = store.get(&id).await.unwrap().unwrap();
    assert!(matches!(
        resolved.resolution(),
        Resolution::Resolved { at: Some(_) }
    ));
    assert!(
        store
            .threads(&ThreadQuery::new(ThreadState::Open, None))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .threads(&ThreadQuery::new(ThreadState::Resolved, None))
            .await
            .unwrap()
            .len(),
        1
    );
    store.set_resolved(&id, false).await.unwrap();
    assert_eq!(
        store.get(&id).await.unwrap().unwrap().resolution(),
        Resolution::Open
    );
    assert!(
        comments_json(&dir)["sets"]["working"]["threads"][0]
            .get("resolvedAt")
            .is_none()
    );

    // The last message takes the thread with it.
    assert_eq!(store.remove_message(&id, &first).await.unwrap(), None);
    assert!(all_threads(&store).await.is_empty());
    let gone = store.remove_thread(&id).await.unwrap_err();
    assert!(
        matches!(gone, CommentError::NotFound(NotFound::Thread(_))),
        "{gone}"
    );
    let missing = store
        .reply(&ThreadId::from("nope".to_owned()), body("x"))
        .await
        .unwrap_err();
    assert!(
        matches!(missing, CommentError::NotFound(NotFound::Thread(_))),
        "{missing}"
    );
    store.add_thread(anchor(), body("hello")).await.unwrap();
    store.clear().await.unwrap();
    assert!(all_threads(&store).await.is_empty());
}

#[tokio::test]
async fn a_missing_message_is_not_found_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let id = store
        .add_thread(anchor(), body("hello"))
        .await
        .unwrap()
        .id()
        .clone();
    let nope = MessageId::from("nope".to_owned());
    let removal = store.remove_message(&id, &nope).await.unwrap_err();
    assert!(
        matches!(removal, CommentError::NotFound(NotFound::Message(_))),
        "{removal}"
    );
    let edit = store
        .edit_message(&id, &nope, MessageText::new("x"))
        .await
        .unwrap_err();
    assert!(
        matches!(edit, CommentError::NotFound(NotFound::Message(_))),
        "{edit}"
    );
    assert_eq!(
        store
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .messages()
            .iter()
            .count(),
        1
    );
}

#[tokio::test]
async fn filters_by_path() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let z = Anchor::Line(line_anchor("z.py", Side::New, 1, "z"));
    store.add_thread(z, body("z")).await.unwrap();
    store.add_thread(anchor(), body("a")).await.unwrap();
    let only_z = store
        .threads(&ThreadQuery::new(ThreadState::All, Some("z.py".into())))
        .await
        .unwrap();
    let paths: Vec<&str> = only_z.iter().map(|t| t.anchor().path().as_str()).collect();
    assert_eq!(paths, ["z.py"]);
    let all: Vec<String> = all_threads(&store)
        .await
        .iter()
        .map(|t| t.anchor().path().to_string())
        .collect();
    assert_eq!(all, ["a.py", "z.py"]);
}

#[tokio::test]
async fn keeps_the_newest_previous_viewed_mark_per_path_and_caps_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    store.set_viewed(entry("a.py", "sha1", true)).await.unwrap();
    let marks = store
        .set_viewed(entry("a.py", "sha1", false))
        .await
        .unwrap();
    assert_eq!(marks, [entry("a.py", "sha1", false)]);
    assert_eq!(store.viewed().await.unwrap(), marks);
    store.set_viewed(entry("a.py", "sha2", true)).await.unwrap();
    store.set_viewed(entry("a.py", "sha3", true)).await.unwrap();
    assert_eq!(
        store.viewed().await.unwrap(),
        [entry("a.py", "sha2", true), entry("a.py", "sha3", true)]
    );
    let many = store
        .set_viewed_many(vec![entry("a.py", "sha4", false)])
        .await
        .unwrap();
    assert_eq!(many, [entry("a.py", "sha4", false)]);
    assert_eq!(store.viewed().await.unwrap(), many);
}
