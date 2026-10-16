//! The snapshot, moving between modes, and the picker's history.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::domains::review::api::api_fixtures::Fixture;
use crate::domains::review::session::session_events::SessionEvent;

#[tokio::test]
async fn the_snapshot_is_served_as_json() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let answer = fixture.get("/api/snapshot").await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers["content-type"], "application/json");
    let snapshot = answer.json();
    assert_eq!(snapshot["version"], json!(1));
    assert_eq!(snapshot["newSha"], json!(fixture.head()));
    assert_eq!(snapshot["changed"][0]["path"], json!("b.txt"));
}

#[tokio::test]
async fn nothing_has_moved_until_a_ref_moves() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    assert_eq!(fixture.get("/api/moved").await.json(), Value::Null);
}

#[tokio::test]
async fn a_reload_recomputes_and_answers_with_the_new_snapshot() {
    let fixture = Fixture::opened(&[]).await;
    assert_eq!(
        fixture.get("/api/snapshot").await.json()["changed"],
        json!([])
    );
    std::fs::write(fixture.repo.path().join("c.txt"), "c\n").unwrap();
    let reloaded = fixture
        .send(Method::POST, "/api/reload", json!({}))
        .await
        .json();
    assert_eq!(reloaded["version"], json!(2));
    assert_eq!(reloaded["changed"][0]["path"], json!("c.txt"));
    assert_eq!(
        fixture.get("/api/snapshot").await.json()["version"],
        json!(2)
    );
}

#[tokio::test]
async fn switching_mode_shows_the_new_review_and_tells_clients() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let mut events = fixture.session.subscribe();
    let switched = fixture
        .send(Method::POST, "/api/mode", json!({"kind": "working"}))
        .await;
    assert_eq!(switched.status, StatusCode::OK, "{}", switched.text());
    assert_eq!(switched.json()["mode"]["commentKey"], json!("working"));
    assert!(matches!(
        events.recv().await.unwrap(),
        SessionEvent::Snapshot(_)
    ));
}

#[tokio::test]
async fn a_revspec_mode_compares_what_it_names() {
    let fixture = Fixture::opened(&[]).await;
    let parent = fixture.parent();
    let switched = fixture
        .send(
            Method::POST,
            "/api/mode",
            json!({"kind": "revspec", "args": [format!("{parent}..HEAD")]}),
        )
        .await
        .json();
    assert_eq!(switched["oldSha"], json!(parent));
    assert_eq!(switched["newSha"], json!(fixture.head()));
}

#[tokio::test]
async fn a_bad_revision_is_the_callers_and_leaves_the_review_as_it_was() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let refused = fixture
        .send(
            Method::POST,
            "/api/mode",
            json!({"kind": "revspec", "args": ["nope..HEAD"]}),
        )
        .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    assert_eq!(refused.json(), json!({"error": "unknown revision: nope"}));
    assert_eq!(
        fixture.get("/api/snapshot").await.json()["version"],
        json!(1)
    );
}

#[tokio::test]
async fn a_mode_this_server_cannot_open_says_so() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let answer = fixture
        .send(Method::POST, "/api/mode", json!({"kind": "pr"}))
        .await;
    assert_eq!(answer.status, StatusCode::NOT_IMPLEMENTED);
    assert!(
        answer.json()["error"]
            .as_str()
            .unwrap()
            .contains("pull request")
    );
}

#[tokio::test]
async fn the_picker_lists_branches_and_recent_commits() {
    let fixture = Fixture::opened(&[]).await;
    fixture.repo.git(&["tag", "v1"]);
    let refs = fixture.get("/api/refs").await.json();
    assert_eq!(refs["current"], json!("main"));
    assert_eq!(refs["branches"], json!(["main"]));
    assert_eq!(refs["tags"], json!(["v1"]));
    assert_eq!(refs["recent"][0]["sha"], json!(fixture.head()));
    assert_eq!(refs["recent"][0]["subject"], json!("edit b.txt"));
}

#[tokio::test]
async fn the_last_commits_preview_reads_both_ends_from_one_head() {
    let fixture = Fixture::opened(&[]).await;
    let preview = fixture
        .get("/api/last-commits-preview?oldOffset=1&newOffset=0")
        .await
        .json();
    assert_eq!(preview["old"]["sha"], json!(fixture.parent()));
    assert_eq!(preview["new"]["sha"], json!(fixture.head()));
    assert_eq!(preview["new"]["message"], json!("edit b.txt"));
    let beyond = fixture
        .get("/api/last-commits-preview?oldOffset=5&newOffset=0")
        .await
        .json();
    assert_eq!(beyond["old"], Value::Null);
    assert_eq!(
        fixture
            .get("/api/last-commits-preview?oldOffset=-1&newOffset=0")
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}
