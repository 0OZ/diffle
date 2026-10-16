//! Listing, exporting and importing threads.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;

use crate::domains::comments::imports::anchor_answers::AnchorQuestions;
use crate::domains::comments::imports::import_wire::parse_imports_value;
use crate::domains::comments::prompt::agent_prompt::format_prompt;
use crate::domains::comments::threads::ids::{MessageId, ThreadId};
use crate::domains::comments::threads::request_wire::{ThreadQueryWire, ThreadStateWire};
use crate::domains::comments::threads::thread::CommentThread;
use crate::domains::comments::threads::thread_query::ThreadQuery;
use crate::domains::comments::threads::thread_wire::CommentThreadWire;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::{ApiQuery, JsonBody};
use crate::domains::review::api::response_api::{json, json_with, text};
use crate::domains::review::session::session_events::SessionEvent;

const NOT_FOUND: &str = "not found";

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/threads", get(list).post(import))
        .route("/api/threads/export", get(export))
        .route(
            "/api/threads/{id}/messages/{message}/export",
            get(export_message),
        )
}

pub fn wires(threads: &[CommentThread]) -> Vec<CommentThreadWire> {
    threads.iter().map(CommentThreadWire::from).collect()
}

async fn list(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<ThreadQueryWire>,
) -> Result<Response, ApiError> {
    let threads = state
        .session()
        .active()?
        .comments()
        .threads(&ThreadQuery::from(query))
        .await?;
    json(&wires(&threads))
}

/// The threads as the prompt an agent is given; only the open ones unless asked otherwise.
async fn export(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<ThreadQueryWire>,
) -> Result<Response, ApiError> {
    let state_filter = query.state.unwrap_or(ThreadStateWire::Open);
    let query = ThreadQuery::new(state_filter.into(), query.path);
    let threads = state.session().active()?.comments().threads(&query).await?;
    Ok(text(format_prompt(&threads)))
}

async fn export_message(
    State(state): State<ApiState>,
    Path((thread, message)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let comments = state.session().active()?.comments();
    let thread = comments
        .get(&ThreadId::from(thread))
        .await?
        .ok_or_else(|| ApiError::NotFound(NOT_FOUND.to_owned()))?;
    let excerpt = thread
        .excerpt(&MessageId::from(message))
        .ok_or_else(|| ApiError::NotFound(NOT_FOUND.to_owned()))?;
    Ok(text(format_prompt(&[excerpt])))
}

/// One thread object or an array of them. Answers with what was created; an open duplicate is
/// skipped.
async fn import(
    State(state): State<ApiState>,
    JsonBody(body): JsonBody<serde_json::Value>,
) -> Result<Response, ApiError> {
    let requests = parse_imports_value(body)?;
    let (active, snapshot) = state.current().await?;
    let answers = state
        .session()
        .anchor_answers(&snapshot, &AnchorQuestions::of(&requests))
        .await?;
    let outcome = active.comments().import_threads(requests, &answers).await?;
    if !outcome.added.is_empty() {
        state.session().announce(SessionEvent::Threads);
    }
    json_with(StatusCode::CREATED, &wires(&outcome.added))
}
