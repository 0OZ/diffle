use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::domains::review::api::api_fixtures::Fixture;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::router_api::router;
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

#[tokio::test]
async fn the_config_starts_with_its_defaults() {
    let fixture = Fixture::opened(&[]).await;
    let config = fixture.get("/api/config").await.json();
    assert_eq!(config["autoViewed"], json!([]));
    assert_eq!(config["contextLines"], json!(ContextLines::default().get()));
    assert_eq!(config["followRefs"], json!(FollowRefs::default().as_str()));
    assert_eq!(config["lspCommands"], json!({}));
}

#[tokio::test]
async fn a_context_change_is_stored_applied_and_announced() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let mut events = fixture.session.subscribe();
    let answer = fixture
        .send(Method::PUT, "/api/config", json!({"contextLines": 1}))
        .await;
    assert_eq!(answer.json()["contextLines"], json!(1));
    assert_eq!(fixture.state.config().get().context_lines.get(), 1);
    assert_eq!(
        fixture.get("/api/snapshot").await.json()["context"],
        json!(1)
    );
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        seen.iter()
            .any(|event| matches!(event, SessionEvent::Config)),
        "{seen:?}"
    );
}

#[tokio::test]
async fn follow_refs_and_auto_viewed_are_stored() {
    let fixture = Fixture::opened(&[]).await;
    let answer = fixture
        .send(
            Method::PUT,
            "/api/config",
            json!({"followRefs": "off", "autoViewed": [" *.lock ", ""]}),
        )
        .await
        .json();
    assert_eq!(answer["followRefs"], json!("off"));
    assert_eq!(answer["autoViewed"], json!(["*.lock"]));
    assert_eq!(fixture.get("/api/config").await.json(), answer);
}

#[tokio::test]
async fn an_update_that_names_nothing_changes_nothing() {
    let fixture = Fixture::opened(&[]).await;
    let before = fixture.get("/api/config").await.json();
    let after = fixture
        .send(Method::PUT, "/api/config", json!({"surprise": true}))
        .await;
    assert_eq!(after.json(), before);
}

#[tokio::test]
async fn an_invalid_update_is_the_callers_and_stores_nothing() {
    let fixture = Fixture::opened(&[]).await;
    let before = fixture.get("/api/config").await.json();
    for body in [
        json!({"contextLines": -1}),
        json!({"contextLines": 1.5}),
        json!({"contextLines": 10001}),
        json!({"followRefs": "sometimes"}),
        json!({"autoViewed": "*.lock"}),
    ] {
        let answer = fixture.send(Method::PUT, "/api/config", body.clone()).await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{body}");
    }
    assert_eq!(fixture.get("/api/config").await.json(), before);
}

#[tokio::test]
async fn globs_given_for_this_run_are_shown_without_being_stored() {
    let fixture = Fixture::opened(&[]).await;
    let state = ApiState::new(
        Arc::clone(fixture.state.session()),
        Arc::clone(fixture.state.config()),
        vec!["dist/**".to_owned()],
    );
    let request = Request::get("/api/config").body(Body::empty()).unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let config: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(config["autoViewed"], json!(["dist/**"]));
    assert!(fixture.state.config().get().auto_viewed.is_empty());
}
