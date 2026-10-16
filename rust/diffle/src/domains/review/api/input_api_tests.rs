use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde::Deserialize;

use super::*;

#[derive(Debug, Deserialize, PartialEq)]
struct Ask {
    count: u32,
}

fn post(body: impl Into<Body>) -> Request<Body> {
    Request::post("/x").body(body.into()).unwrap()
}

#[tokio::test]
async fn a_body_is_read_as_json_whatever_its_content_type() {
    let JsonBody(ask) = JsonBody::<Ask>::from_request(post(r#"{"count":2}"#), &())
        .await
        .unwrap_or_else(|_| panic!("a valid body"));
    assert_eq!(ask, Ask { count: 2 });
}

#[tokio::test]
async fn broken_json_and_a_wrong_shape_are_different_refusals() {
    let Err(syntax) = JsonBody::<Ask>::from_request(post("{"), &()).await else {
        panic!("expected a refusal");
    };
    assert_eq!(syntax.to_string(), "invalid JSON body");
    let Err(shape) = JsonBody::<Ask>::from_request(post(r#"{"count":"x"}"#), &()).await else {
        panic!("expected a refusal");
    };
    assert_eq!(shape.status(), StatusCode::BAD_REQUEST);
    assert!(shape.to_string().contains("invalid type"), "{shape}");
}

#[tokio::test]
async fn a_body_past_the_limit_is_too_large() {
    let big = vec![b' '; MAX_BODY_BYTES + 1];
    let Err(error) = JsonBody::<Ask>::from_request(post(big), &()).await else {
        panic!("expected a refusal");
    };
    assert_eq!(error.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn a_query_is_read_into_its_type_or_refused() {
    let (mut parts, ()) = Request::get("/x?count=3").body(()).unwrap().into_parts();
    let ApiQuery(ask) = ApiQuery::<Ask>::from_request_parts(&mut parts, &())
        .await
        .unwrap_or_else(|_| panic!("a valid query"));
    assert_eq!(ask, Ask { count: 3 });
    let (mut parts, ()) = Request::get("/x?count=many").body(()).unwrap().into_parts();
    assert!(
        ApiQuery::<Ask>::from_request_parts(&mut parts, &())
            .await
            .is_err()
    );
}
