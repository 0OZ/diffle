use axum::body::to_bytes;
use serde_json::json;

use super::*;

#[tokio::test]
async fn json_answers_with_its_status_type_and_bytes() {
    let response = json_with(StatusCode::CREATED, &json!({"a": 1})).unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], br#"{"a":1}"#);
}

#[tokio::test]
async fn text_and_nothing_have_their_own_shape() {
    let response = text("hello");
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=UTF-8"
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"hello");
    assert_eq!(no_content().status(), StatusCode::NO_CONTENT);
}
