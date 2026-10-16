use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tower::ServiceExt;

use super::*;

fn app() -> Router {
    let guard = Arc::new(RequestGuard::new("127.0.0.1", &[], None));
    Router::new()
        .route(
            "/api/x",
            get(|| async { "ok" }).layer(from_fn_with_state(Arc::clone(&guard), guard_api)),
        )
        .route(
            "/ws",
            get(|| async { "ok" }).layer(from_fn_with_state(guard, guard_socket)),
        )
}

async fn call(
    uri: &str,
    host: Option<&str>,
    origin: Option<&str>,
) -> (StatusCode, String, Option<String>) {
    let mut request = Request::get(uri);
    if let Some(host) = host {
        request = request.header(header::HOST, host);
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let response = app()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let kind = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|value| value.to_str().unwrap().to_owned());
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap(), kind)
}

#[tokio::test]
async fn a_local_host_passes_through() {
    let (status, body, _) = call(
        "/api/x",
        Some("127.0.0.1:4966"),
        Some("http://127.0.0.1:4966"),
    )
    .await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "ok"));
    assert_eq!(
        call("/ws", Some("localhost:4966"), None).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn the_api_rejects_with_json_naming_the_header() {
    let (status, body, kind) = call("/api/x", Some("evil.example"), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(kind.as_deref(), Some("application/json"));
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .starts_with("forbidden Host \"evil.example\""),
        "{body}"
    );
}

#[tokio::test]
async fn a_foreign_origin_is_rejected_even_from_a_local_host() {
    let (status, body, _) = call(
        "/api/x",
        Some("127.0.0.1:4966"),
        Some("https://evil.example"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("forbidden Origin"), "{body}");
}

#[tokio::test]
async fn the_socket_rejects_with_plain_text() {
    let (status, body, kind) = call("/ws", None, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(kind.as_deref(), Some("text/plain"));
    assert_eq!(body, "missing Host header\n");
}
