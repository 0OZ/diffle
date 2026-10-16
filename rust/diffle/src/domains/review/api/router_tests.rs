//! What the whole route table promises: the gate, the errors, and the routes not built yet.

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::api_fixtures::Fixture;
use super::input_api::MAX_BODY_BYTES;

const BRIEFLY: Duration = Duration::from_millis(150);
const PATIENCE: Duration = Duration::from_secs(10);

#[tokio::test]
async fn a_request_waits_for_the_first_review_but_the_config_does_not() {
    let fixture = Fixture::two_commits().await;
    assert_eq!(fixture.get("/api/config").await.status, StatusCode::OK);

    let pending = fixture.get("/api/snapshot");
    tokio::pin!(pending);
    assert!(tokio::time::timeout(BRIEFLY, &mut pending).await.is_err());

    let comparison = fixture.comparison(&["HEAD^!"]).await;
    fixture.session.start(comparison).await.unwrap();
    let answer = tokio::time::timeout(PATIENCE, pending).await.unwrap();
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.json()["version"], json!(1));
}

#[tokio::test]
async fn an_unknown_api_route_names_the_likely_cause() {
    let fixture = Fixture::opened(&[]).await;
    let answer = fixture.get("/api/teleport").await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.json(),
        json!({"error": "API endpoint not found. Restart diffle to load the latest server."})
    );
}

#[tokio::test]
async fn a_body_that_is_not_json_is_refused_with_the_clients_wording() {
    let fixture = Fixture::opened(&[]).await;
    let request = axum::http::Request::post("/api/mode")
        .body(axum::body::Body::from("{"))
        .unwrap();
    let response = tower::ServiceExt::oneshot(fixture.router(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&bytes[..], br#"{"error":"invalid JSON body"}"#);
}

#[tokio::test]
async fn a_body_over_a_mebibyte_is_too_large() {
    let fixture = Fixture::opened(&[]).await;
    let big = Value::String("x".repeat(MAX_BODY_BYTES));
    let answer = fixture.send(Method::POST, "/api/mode", big).await;
    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(answer.json(), json!({"error": "request body too large"}));
}

#[tokio::test]
async fn github_is_reported_as_unavailable_not_as_an_error() {
    let fixture = Fixture::opened(&[]).await;
    let github = fixture.get("/api/github").await.json();
    assert_eq!(github["version"], json!(1));
    assert_eq!(github["repository"], Value::Null);
    assert_eq!(github["pullRequest"], Value::Null);
    assert!(github["reason"].as_str().unwrap().contains("not available"));
    let export = fixture
        .send(Method::POST, "/api/github/export", json!({}))
        .await;
    assert_eq!(export.status, StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn language_servers_are_off_and_symbol_requests_say_so() {
    let fixture = Fixture::opened(&[]).await;
    assert_eq!(
        fixture.get("/api/lsp/status").await.json(),
        json!({"enabled": false, "servers": [], "missing": []})
    );
    for route in [
        "definition",
        "type-definition",
        "hover",
        "token-kind",
        "references",
        "occurrences",
    ] {
        let answer = fixture
            .send(Method::POST, &format!("/api/lsp/{route}"), json!({}))
            .await;
        assert_eq!(answer.status, StatusCode::CONFLICT, "{route}");
    }
    assert_eq!(
        fixture.get("/api/lsp/symbols?q=x").await.status,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn recorded_iterations_cannot_be_deleted_here() {
    let fixture = Fixture::opened(&[]).await;
    assert_eq!(
        fixture.delete("/api/iterations").await.status,
        StatusCode::NOT_IMPLEMENTED
    );
    assert_eq!(
        fixture.delete("/api/iterations/2").await.status,
        StatusCode::NOT_IMPLEMENTED
    );
}
