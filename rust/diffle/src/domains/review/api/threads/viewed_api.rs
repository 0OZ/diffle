//! `GET /api/viewed`, `PUT /api/viewed`, `PUT /api/viewed/bulk`: which files are done with.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::{get, put};

use crate::domains::comments::viewed::viewed_marks::ViewedEntry;
use crate::domains::comments::viewed::viewed_wire::{ViewedBulkWire, ViewedEntryWire};
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::JsonBody;
use crate::domains::review::api::response_api::json;
use crate::domains::review::session::session_events::SessionEvent;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/viewed", get(marks).put(mark))
        .route("/api/viewed/bulk", put(mark_many))
}

fn wires(entries: &[ViewedEntry]) -> Vec<ViewedEntryWire> {
    entries.iter().map(ViewedEntryWire::from).collect()
}

async fn marks(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let entries = state.session().active()?.comments().viewed().await?;
    json(&wires(&entries))
}

async fn mark(
    State(state): State<ApiState>,
    JsonBody(wire): JsonBody<ViewedEntryWire>,
) -> Result<Response, ApiError> {
    let entry = ViewedEntry::try_from(wire)?;
    let entries = state
        .session()
        .active()?
        .comments()
        .set_viewed(entry)
        .await?;
    state.session().announce(SessionEvent::Viewed);
    json(&wires(&entries))
}

async fn mark_many(
    State(state): State<ApiState>,
    JsonBody(wire): JsonBody<ViewedBulkWire>,
) -> Result<Response, ApiError> {
    let entries = Vec::<ViewedEntry>::try_from(wire)?;
    let entries = state
        .session()
        .active()?
        .comments()
        .set_viewed_many(entries)
        .await?;
    state.session().announce(SessionEvent::Viewed);
    json(&wires(&entries))
}
