//! Importing, reading, editing and exporting threads, and marking files viewed.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::domains::review::api::api_fixtures::{Answer, Fixture};
use crate::domains::review::session::session_events::SessionEvent;

async fn with_thread() -> (Fixture, String) {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let created = fixture
        .send(
            Method::POST,
            "/api/threads",
            json!({"path": "b.txt", "startLine": 1, "body": "why b?"}),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text());
    let id = created.json()[0]["id"].as_str().unwrap().to_owned();
    (fixture, id)
}

fn thread_of(answer: &Answer) -> Value {
    answer.json()
}

#[tokio::test]
async fn an_imported_thread_is_quoted_from_the_snapshot_and_announced() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let mut events = fixture.session.subscribe();
    let created = fixture
        .send(
            Method::POST,
            "/api/threads",
            json!({"path": "b.txt", "startLine": 1, "body": "why b?"}),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let thread = &created.json()[0];
    assert_eq!(thread["anchor"]["path"], json!("b.txt"));
    assert_eq!(thread["anchor"]["quoted"], json!("b"));
    assert_eq!(thread["messages"][0]["body"], json!("why b?"));
    assert_eq!(thread["resolved"], json!(false));
    assert!(matches!(
        events.recv().await.unwrap(),
        SessionEvent::Threads
    ));
}

#[tokio::test]
async fn importing_an_open_duplicate_adds_nothing_and_says_nothing() {
    let (fixture, _) = with_thread().await;
    let mut events = fixture.session.subscribe();
    let again = fixture
        .send(
            Method::POST,
            "/api/threads",
            json!({"path": "b.txt", "startLine": 1, "body": "why b?"}),
        )
        .await;
    assert_eq!(again.status, StatusCode::CREATED);
    assert_eq!(again.json(), json!([]));
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn a_bad_import_is_the_callers() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let post = |body: Value| fixture.send(Method::POST, "/api/threads", body);
    assert_eq!(
        post(json!({"path": "b.txt", "body": " "})).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(json!({"path": "b.txt", "startLine": 99, "body": "x"}))
            .await
            .status,
        StatusCode::BAD_REQUEST,
        "a line the file does not have cannot be quoted"
    );
}

#[tokio::test]
async fn threads_are_listed_by_state_and_path() {
    let (fixture, id) = with_thread().await;
    assert_eq!(
        fixture
            .get("/api/threads")
            .await
            .json()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fixture.get("/api/threads?state=resolved").await.json(),
        json!([])
    );
    assert_eq!(
        fixture.get("/api/threads?path=a.txt").await.json(),
        json!([])
    );
    let resolved = fixture
        .send(
            Method::PUT,
            &format!("/api/threads/{id}/resolved"),
            json!({"resolved": true}),
        )
        .await;
    assert_eq!(thread_of(&resolved)["resolved"], json!(true));
    assert_eq!(
        fixture
            .get("/api/threads?state=resolved")
            .await
            .json()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fixture.get("/api/threads?state=open").await.json(),
        json!([])
    );
    assert_eq!(
        fixture.get("/api/threads?state=done").await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn replies_are_added_edited_and_removed() {
    let (fixture, id) = with_thread().await;
    let reply = fixture
        .send(
            Method::POST,
            &format!("/api/threads/{id}/replies"),
            json!({"body": "because"}),
        )
        .await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let message = reply.json()["messages"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let edited = fixture
        .send(
            Method::PATCH,
            &format!("/api/threads/{id}/messages/{message}"),
            json!({"body": "because b"}),
        )
        .await;
    assert_eq!(edited.json()["messages"][1]["body"], json!("because b"));

    let removed = fixture
        .delete(&format!("/api/threads/{id}/messages/{message}"))
        .await;
    assert_eq!(removed.json()["messages"].as_array().unwrap().len(), 1);

    let first = edited.json()["messages"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let last = fixture
        .delete(&format!("/api/threads/{id}/messages/{first}"))
        .await;
    assert_eq!(
        last.status,
        StatusCode::NO_CONTENT,
        "the last message takes the thread"
    );
    assert_eq!(fixture.get("/api/threads").await.json(), json!([]));
}

#[tokio::test]
async fn a_blank_reply_is_refused_and_an_unknown_thread_is_not_found() {
    let (fixture, id) = with_thread().await;
    let blank = fixture
        .send(
            Method::POST,
            &format!("/api/threads/{id}/replies"),
            json!({"body": "  "}),
        )
        .await;
    assert_eq!(blank.status, StatusCode::BAD_REQUEST);
    assert_eq!(blank.json(), json!({"error": "body required"}));
    let missing = fixture
        .send(
            Method::POST,
            "/api/threads/nope/replies",
            json!({"body": "x"}),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json(), json!({"error": "not found"}));
}

#[tokio::test]
async fn threads_are_removed_one_stale_or_all() {
    let (fixture, id) = with_thread().await;
    assert_eq!(
        fixture.delete(&format!("/api/threads/{id}")).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture.delete(&format!("/api/threads/{id}")).await.status,
        StatusCode::NOT_FOUND
    );

    let (fixture, _) = with_thread().await;
    assert_eq!(
        fixture.delete("/api/threads?stale=1").await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        fixture
            .get("/api/threads")
            .await
            .json()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        fixture.delete("/api/threads?stale=2").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fixture.delete("/api/threads").await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(fixture.get("/api/threads").await.json(), json!([]));
}

#[tokio::test]
async fn threads_export_as_the_prompt_an_agent_reads() {
    let (fixture, id) = with_thread().await;
    let export = fixture.get("/api/threads/export").await;
    assert_eq!(export.headers["content-type"], "text/plain; charset=UTF-8");
    assert_eq!(export.text(), "b.txt:1\n\n> b\n\nwhy b?\n\n---\n");
    fixture
        .send(
            Method::PUT,
            &format!("/api/threads/{id}/resolved"),
            json!({"resolved": true}),
        )
        .await;
    assert_eq!(
        fixture.get("/api/threads/export").await.text(),
        "",
        "only open threads by default"
    );
    assert!(
        fixture
            .get("/api/threads/export?state=all")
            .await
            .text()
            .contains("why b?")
    );
}

#[tokio::test]
async fn one_message_of_a_conversation_exports_alone() {
    let (fixture, id) = with_thread().await;
    let reply = fixture
        .send(
            Method::POST,
            &format!("/api/threads/{id}/replies"),
            json!({"body": "because"}),
        )
        .await;
    let message = reply.json()["messages"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let text = fixture
        .get(&format!("/api/threads/{id}/messages/{message}/export"))
        .await
        .text();
    assert!(
        text.contains("because") && !text.contains("why b?"),
        "{text}"
    );
    let unknown = fixture
        .get(&format!("/api/threads/{id}/messages/nope/export"))
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn viewed_marks_are_kept_and_announced() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let mut events = fixture.session.subscribe();
    let entry = json!({"path": "b.txt", "blob": "abc", "viewed": true});
    let marked = fixture
        .send(Method::PUT, "/api/viewed", entry.clone())
        .await;
    assert_eq!(marked.json(), json!([entry]));
    assert!(matches!(events.recv().await.unwrap(), SessionEvent::Viewed));
    assert_eq!(fixture.get("/api/viewed").await.json(), json!([entry]));

    let bulk = json!({"entries": [{"path": "a.txt", "blob": "def", "viewed": true}]});
    let replaced = fixture.send(Method::PUT, "/api/viewed/bulk", bulk).await;
    assert_eq!(replaced.status, StatusCode::OK);
    assert_eq!(
        fixture
            .send(
                Method::PUT,
                "/api/viewed",
                json!({"path": "", "blob": "x", "viewed": true})
            )
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}
