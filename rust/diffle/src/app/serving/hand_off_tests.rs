use axum::http::Method;
use serde_json::json;

use super::*;
use crate::domains::review::api::api_fixtures::Fixture;

#[tokio::test]
async fn the_open_comments_become_the_prompt() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    fixture
        .send(
            Method::POST,
            "/api/threads",
            json!({"path": "b.txt", "startLine": 1, "body": "why b?"}),
        )
        .await;
    let prompt = open_threads_prompt(&fixture.session).await;
    assert!(prompt.contains("b.txt"), "{prompt}");
    assert!(prompt.contains("why b?"), "{prompt}");
}

#[tokio::test]
async fn resolved_comments_are_not_handed_over() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let created = fixture
        .send(
            Method::POST,
            "/api/threads",
            json!({"path": "b.txt", "startLine": 1, "body": "why b?"}),
        )
        .await;
    let id = created.json()[0]["id"].as_str().unwrap().to_owned();
    fixture
        .send(
            Method::PUT,
            &format!("/api/threads/{id}/resolved"),
            json!({"resolved": true}),
        )
        .await;
    assert_eq!(open_threads_prompt(&fixture.session).await, "");
}

#[tokio::test]
async fn a_review_that_never_opened_has_nothing_to_hand_over() {
    let fixture = Fixture::two_commits().await;
    assert_eq!(open_threads_prompt(&fixture.session).await, "");
}
