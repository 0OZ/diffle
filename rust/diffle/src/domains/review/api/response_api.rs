//! Answers that are not errors: JSON serialized once, plain text, nothing.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::error_api::ApiError;

const JSON: &str = "application/json";
const TEXT: &str = "text/plain; charset=UTF-8";

/// `value` as a JSON body. Serialized straight to bytes, so a snapshot that borrows from its
/// owner never has to outlive the handler.
pub fn json<T: Serialize>(value: &T) -> Result<Response, ApiError> {
    json_with(StatusCode::OK, value)
}

pub fn json_with<T: Serialize>(status: StatusCode, value: &T) -> Result<Response, ApiError> {
    let bytes = serde_json::to_vec(value).map_err(|error| ApiError::Internal(error.to_string()))?;
    Ok((status, [(header::CONTENT_TYPE, JSON)], bytes).into_response())
}

#[must_use]
pub fn text(body: impl Into<String>) -> Response {
    (StatusCode::OK, [(header::CONTENT_TYPE, TEXT)], body.into()).into_response()
}

#[must_use]
pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
#[path = "response_api_tests.rs"]
mod tests;
