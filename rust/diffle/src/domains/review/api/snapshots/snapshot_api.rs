//! `GET /api/snapshot`, `GET /api/moved`, `POST /api/reload`.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::{get, post};

use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::response_api::json;
use crate::domains::review::api::wire::server_message_wire::MovedWire;
use crate::domains::review::api::wire::snapshot_wire::SnapshotWire;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/snapshot", get(snapshot))
        .route("/api/moved", get(moved))
        .route("/api/reload", post(reload))
}

async fn snapshot(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let (_, snapshot) = state.current().await?;
    json(&SnapshotWire::from(&*snapshot))
}

/// The pending reload of a refs-live comparison, for a client that connects after the push.
async fn moved(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let moved = state.session().active()?.moved();
    json(&moved.as_ref().map(MovedWire::from))
}

async fn reload(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let snapshot = state.session().reload().await?;
    json(&SnapshotWire::from(&*snapshot))
}
