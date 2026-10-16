//! Turning away requests whose `Host` or `Origin` is not trusted, before any route sees them.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::kernel::request_guard::{Rejection, RequestGuard};

#[derive(Serialize)]
struct RejectedBody {
    error: String,
}

fn verdict(guard: &RequestGuard, headers: &HeaderMap) -> Result<(), Rejection> {
    let header_text = |name| headers.get(name).and_then(|value| value.to_str().ok());
    guard.check(header_text(header::HOST), header_text(header::ORIGIN))
}

/// For the API: a rejection is JSON like every other API error.
pub async fn guard_api(
    State(guard): State<Arc<RequestGuard>>,
    request: Request,
    next: Next,
) -> Response {
    match verdict(&guard, request.headers()) {
        Ok(()) => next.run(request).await,
        Err(rejection) => (
            StatusCode::FORBIDDEN,
            Json(RejectedBody {
                error: rejection.to_string(),
            }),
        )
            .into_response(),
    }
}

/// For the socket upgrade: a browser's console shows a plain-text reason, which names the
/// failing header where a bare hang-up would leave it blank.
pub async fn guard_socket(
    State(guard): State<Arc<RequestGuard>>,
    request: Request,
    next: Next,
) -> Response {
    match verdict(&guard, request.headers()) {
        Ok(()) => next.run(request).await,
        Err(rejection) => (
            StatusCode::FORBIDDEN,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("{rejection}\n"),
        )
            .into_response(),
    }
}

#[cfg(test)]
#[path = "guard_layer_tests.rs"]
mod tests;
