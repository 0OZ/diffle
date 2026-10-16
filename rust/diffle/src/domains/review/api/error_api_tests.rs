use axum::body::to_bytes;

use super::*;
use crate::domains::comments::threads::ids::ThreadId;
use crate::domains::repository::process::git_runner::Exit;

async fn answer(error: ApiError) -> (StatusCode, String) {
    let response = error.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn an_error_answers_json_with_its_message() {
    let (status, body) = answer(ApiError::bad_request("nope")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, r#"{"error":"nope"}"#);
}

#[tokio::test]
async fn each_kind_has_its_status() {
    let cases = [
        (ApiError::NotFound("x".into()), StatusCode::NOT_FOUND),
        (ApiError::Conflict("x".into()), StatusCode::CONFLICT),
        (ApiError::TooLarge, StatusCode::PAYLOAD_TOO_LARGE),
        (ApiError::not_implemented("pr"), StatusCode::NOT_IMPLEMENTED),
        (
            ApiError::Internal("x".into()),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];
    for (error, status) in cases {
        assert_eq!(error.status(), status);
    }
    let (_, body) = answer(ApiError::not_implemented("pull requests")).await;
    assert_eq!(
        body,
        r#"{"error":"pull requests is not available in this server yet"}"#
    );
}

#[test]
fn git_refusing_is_the_callers_and_losing_git_is_ours() {
    let refused = GitError::Failed {
        command: "grep".into(),
        exit: Exit::Failure(128),
        stderr: "bad regex".into(),
    };
    assert_eq!(ApiError::from(refused).status(), StatusCode::BAD_REQUEST);
    let lost = GitError::Spawn(std::io::Error::other("gone"));
    assert_eq!(
        ApiError::from(lost).status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn comment_refusals_map_to_not_found_and_bad_request() {
    let missing = CommentError::from(NotFound::Thread(ThreadId::from("t".to_owned())));
    assert_eq!(ApiError::from(missing).status(), StatusCode::NOT_FOUND);
    assert_eq!(
        ApiError::from(ReviewError::NothingToFocus).status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ApiError::from(ReviewError::NotOpen).status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        ApiError::from(BodyError::Blank).to_string(),
        "body required"
    );
}
