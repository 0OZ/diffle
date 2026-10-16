use std::fs as stdfs;

use serde_json::{Value, json};

use super::*;
use crate::domains::comments::sample_threads::line;
use crate::domains::comments::storage::scratch_store::{
    all_threads, anchor, body, comments_json, entry, key, open,
};
use crate::domains::comments::threads::thread::{Freshness, Resolution};
use crate::kernel::epoch_millis::EpochMillis;

#[tokio::test]
async fn migrates_a_v1_file_once_keeping_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    let anchor = json!({"kind": "line", "path": "a.py", "side": "new", "startLine": 2, "endLine": 2, "quoted": "b"});
    let v1 = json!({
        "version": 1,
        "sets": {"working": {
            "comments": [{"id": "c1", "anchor": anchor, "body": "old note", "createdAt": 5, "updatedAt": 6, "stale": true, "staleFromLine": 9}],
            "viewed": [{"path": "a.py", "blob": "sha", "viewed": true}]
        }}
    });
    stdfs::write(dir.path().join("diffle/comments.json"), v1.to_string()).unwrap();
    let store = open(&dir, "working").await;
    let threads = all_threads(&store).await;
    let [thread] = threads.as_slice() else {
        panic!("one thread")
    };
    assert_eq!(thread.id().as_str(), "c1");
    assert!(thread.is_stale() && !thread.is_resolved());
    assert_eq!(
        thread.freshness(),
        Freshness::Stale {
            from_line: Some(line(9))
        }
    );
    assert_eq!(thread.messages().iter().count(), 1);
    let message = thread.messages().first();
    assert_eq!(message.text().as_str(), "old note");
    assert_eq!(
        (message.created_at().get(), message.updated_at().get()),
        (5, 6)
    );
    assert_eq!(store.viewed().await.unwrap(), [entry("a.py", "sha", true)]);
    let backup = stdfs::read_to_string(dir.path().join("diffle/comments.v1.bak")).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&backup).unwrap(), v1);
    store.reply(thread.id(), body("reply")).await.unwrap();
    let raw = comments_json(&dir);
    assert_eq!(raw["version"], 3);
    assert_eq!(
        raw["sets"]["working"]["threads"][0]["messages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
async fn migrates_a_v2_file_once_stamping_its_line_anchors_and_keeping_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    let v2_anchor =
        json!({"path": "a.py", "side": "new", "startLine": 2, "endLine": 2, "quoted": "b"});
    let v2 = json!({
        "version": 2,
        "sets": {
            "working": {
                "threads": [{
                    "id": "t1", "anchor": v2_anchor,
                    "messages": [{"id": "m1", "body": "note", "createdAt": 5, "updatedAt": 6}],
                    "resolved": true, "resolvedAt": 7, "stale": false
                }],
                "viewed": [{"path": "a.py", "blob": "sha", "viewed": true}]
            },
            "pr:abc": {"threads": []}
        }
    });
    stdfs::write(dir.path().join("diffle/comments.json"), v2.to_string()).unwrap();
    let store = open(&dir, "working").await;
    let mut expected = v2["sets"]["working"]["threads"][0].clone();
    expected["anchor"]["kind"] = json!("line");
    let raw = comments_json(&dir);
    assert_eq!(raw["sets"]["working"]["threads"][0], expected);
    let thread = store
        .get(&ThreadId::from("t1".to_owned()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        thread.resolution(),
        Resolution::Resolved {
            at: Some(EpochMillis::new(7))
        }
    );
    assert_eq!(store.viewed().await.unwrap(), [entry("a.py", "sha", true)]);
    let backup = stdfs::read_to_string(dir.path().join("diffle/comments.v2.bak")).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&backup).unwrap(), v2);
    assert_eq!(raw["version"], 3);
    let keys: Vec<&String> = raw["sets"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["pr:abc", "working"]);
}

#[tokio::test]
async fn a_current_or_unknown_file_is_left_alone_and_a_malformed_one_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    let file = dir.path().join("diffle/comments.json");
    stdfs::write(&file, r#"{"version":9,"sets":{}}"#).unwrap();
    open(&dir, "working").await;
    assert_eq!(
        stdfs::read_to_string(&file).unwrap(),
        r#"{"version":9,"sets":{}}"#
    );
    assert!(!dir.path().join("diffle/comments.v9.bak").exists());
    stdfs::write(&file, "{not json").unwrap();
    let error = CommentStore::open(dir.path(), &key("working"))
        .await
        .unwrap_err();
    assert!(matches!(error, CommentError::Malformed { .. }), "{error}");
}

#[tokio::test]
async fn a_stored_thread_breaking_a_domain_rule_is_reported_not_dropped() {
    let dir = tempfile::tempdir().unwrap();
    stdfs::create_dir_all(dir.path().join("diffle")).unwrap();
    let thread = json!({
        "id": "t1", "anchor": {"kind": "file", "path": "a.py"}, "messages": [], "resolved": false, "stale": false
    });
    let raw = json!({"version": 3, "sets": {"working": {"threads": [thread], "viewed": []}}});
    stdfs::write(dir.path().join("diffle/comments.json"), raw.to_string()).unwrap();
    let store = open(&dir, "working").await;
    let error = store.threads(&ThreadQuery::all()).await.unwrap_err();
    assert!(matches!(error, CommentError::Invalid { .. }), "{error}");
    assert!(store.add_thread(anchor(), body("x")).await.is_err());
    assert_eq!(comments_json(&dir), raw);
}
