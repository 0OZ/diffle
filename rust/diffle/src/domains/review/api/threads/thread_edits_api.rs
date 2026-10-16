//! Changing a thread: reply, edit, resolve, remove.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{delete, put};
use serde::Deserialize;

use crate::domains::comments::threads::body::{Body, MessageText};
use crate::domains::comments::threads::ids::{MessageId, ThreadId};
use crate::domains::comments::threads::request_wire::{
    MessagePatchWire, ReplyWire, ResolvedRequestWire,
};
use crate::domains::comments::threads::thread_wire::CommentThreadWire;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::{ApiQuery, JsonBody};
use crate::domains::review::api::response_api::{json, json_with, no_content};
use crate::domains::review::session::session_events::SessionEvent;

/// `?stale=1` is the only value a clear request takes.
#[derive(Debug, Deserialize)]
enum StaleOnly {
    #[serde(rename = "1")]
    On,
}

#[derive(Debug, Deserialize)]
struct ClearQuery {
    stale: Option<StaleOnly>,
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/threads", delete(clear))
        .route("/api/threads/{id}", delete(remove))
        .route("/api/threads/{id}/replies", axum::routing::post(reply))
        .route(
            "/api/threads/{id}/messages/{message}",
            axum::routing::patch(edit_message).delete(remove_message),
        )
        .route("/api/threads/{id}/resolved", put(resolve))
}

fn threads_changed(state: &ApiState) {
    state.session().announce(SessionEvent::Threads);
}

async fn reply(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    JsonBody(wire): JsonBody<ReplyWire>,
) -> Result<Response, ApiError> {
    let body = Body::try_from(wire)?;
    let thread = state
        .session()
        .active()?
        .comments()
        .reply(&ThreadId::from(id), body)
        .await?;
    threads_changed(&state);
    json_with(StatusCode::CREATED, &CommentThreadWire::from(&thread))
}

async fn edit_message(
    State(state): State<ApiState>,
    Path((id, message)): Path<(String, String)>,
    JsonBody(wire): JsonBody<MessagePatchWire>,
) -> Result<Response, ApiError> {
    let thread = state
        .session()
        .active()?
        .comments()
        .edit_message(
            &ThreadId::from(id),
            &MessageId::from(message),
            MessageText::from(wire),
        )
        .await?;
    threads_changed(&state);
    json(&CommentThreadWire::from(&thread))
}

/// Answers with the thread that is left, or nothing when that was its last message.
async fn remove_message(
    State(state): State<ApiState>,
    Path((id, message)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let left = state
        .session()
        .active()?
        .comments()
        .remove_message(&ThreadId::from(id), &MessageId::from(message))
        .await?;
    threads_changed(&state);
    match left {
        Some(thread) => json(&CommentThreadWire::from(&thread)),
        None => Ok(no_content()),
    }
}

async fn resolve(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    JsonBody(wire): JsonBody<ResolvedRequestWire>,
) -> Result<Response, ApiError> {
    let thread = state
        .session()
        .active()?
        .comments()
        .set_resolved(&ThreadId::from(id), wire.resolved)
        .await?;
    threads_changed(&state);
    json(&CommentThreadWire::from(&thread))
}

async fn remove(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    state
        .session()
        .active()?
        .comments()
        .remove_thread(&ThreadId::from(id))
        .await?;
    threads_changed(&state);
    Ok(no_content())
}

/// Every thread of this mode, or with `?stale=1` only the stale ones.
async fn clear(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<ClearQuery>,
) -> Result<Response, ApiError> {
    let comments = state.session().active()?.comments();
    match query.stale {
        Some(StaleOnly::On) => comments.remove_stale().await.map(drop)?,
        None => comments.clear().await?,
    }
    threads_changed(&state);
    Ok(no_content())
}
