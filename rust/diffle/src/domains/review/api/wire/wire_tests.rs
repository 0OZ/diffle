//! The shapes the client reads, against a real repository.

use serde_json::{Value, json};

use super::server_message_wire::ServerMessageWire;
use super::snapshot_wire::SnapshotWire;
use crate::domains::review::api::api_fixtures::{Fixture, json_of};
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;

fn message(event: &SessionEvent) -> Value {
    json_of(&ServerMessageWire::from(event))
}

#[tokio::test]
async fn a_commit_snapshot_has_every_field_of_the_protocol() {
    let fixture = Fixture::two_commits().await;
    let snapshot = fixture
        .session
        .start(fixture.comparison(&["HEAD^!"]).await)
        .await
        .unwrap();
    let wire = json_of(&SnapshotWire::from(&*snapshot));

    let head = fixture.head();
    let parent = fixture.parent();
    assert_eq!(wire["root"], json!(fixture.root()));
    assert_eq!(wire["version"], json!(1));
    assert_eq!(wire["oldSha"], json!(parent));
    assert_eq!(wire["newSha"], json!(head));
    assert_eq!(wire["headSha"], json!(head));
    assert_eq!(wire["context"], json!(5));
    assert_eq!(wire["iterations"], json!([]));
    assert_eq!(wire["tree"], json!(["a.txt", "b.txt"]));
    assert_eq!(wire["mode"]["base"], json!("parent"));
    assert_eq!(wire["mode"]["live"], json!("none"));
    assert_eq!(wire["mode"]["old"], wire["mode"]["new"]);
    assert_eq!(wire["mode"]["commentKey"], json!(format!("commit:{head}")));
    assert!(wire["mode"].get("within").is_none());
    assert_eq!(wire["commit"]["sha"], json!(head));
    assert_eq!(wire["commit"]["parent"], json!(parent));
    assert_eq!(wire["commit"]["child"], Value::Null);
    assert_eq!(wire["commits"]["total"], json!(1));
    assert_eq!(wire["commits"]["list"][0]["author"], json!("Test"));
    assert_eq!(wire["commits"]["oldSha"], json!(parent));
}

#[tokio::test]
async fn a_changed_file_carries_its_counts_and_blobs() {
    let fixture = Fixture::two_commits().await;
    let snapshot = fixture
        .session
        .start(fixture.comparison(&["HEAD^!"]).await)
        .await
        .unwrap();
    let wire = json_of(&SnapshotWire::from(&*snapshot));
    let file = &wire["changed"][0];
    assert_eq!(file["path"], json!("b.txt"));
    assert_eq!(file["status"], json!("A"));
    assert_eq!(file["additions"], json!(1));
    assert_eq!(file["deletions"], json!(0));
    assert_eq!(file["binary"], json!(false));
    assert_eq!(file["oldBlob"], json!(""));
    assert_eq!(file["generated"], json!(false));
    assert_eq!(file["blob"].as_str().map(str::len), Some(40));
    assert!(file.get("oldPath").is_none());
    assert!(file.get("submodule").is_none());
}

#[tokio::test]
async fn the_working_review_is_live_and_has_no_commit() {
    let fixture = Fixture::two_commits().await;
    let snapshot = fixture
        .session
        .start(fixture.comparison(&[]).await)
        .await
        .unwrap();
    let wire = json_of(&SnapshotWire::from(&*snapshot));
    assert_eq!(wire["newSha"], json!("worktree"));
    assert_eq!(wire["mode"]["live"], json!("worktree"));
    assert_eq!(wire["mode"]["commentKey"], json!("working"));
    assert_eq!(wire["mode"]["base"], json!("direct"));
    assert_eq!(wire["commit"], Value::Null);
}

#[test]
fn server_messages_use_the_protocol_spelling() {
    let version = SnapshotVersion::default().next();
    assert_eq!(
        message(&SessionEvent::Snapshot(version)),
        json!({"type": "snapshot", "version": 1})
    );
    assert_eq!(
        message(&SessionEvent::Moved(None)),
        json!({"type": "moved", "moved": null})
    );
    for (event, kind) in [
        (SessionEvent::Threads, "threads"),
        (SessionEvent::Viewed, "viewed"),
        (SessionEvent::Config, "config"),
    ] {
        assert_eq!(message(&event), json!({ "type": kind }));
    }
}
