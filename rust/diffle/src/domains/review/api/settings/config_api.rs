//! `GET /api/config` and `PUT /api/config`.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;

use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::JsonBody;
use crate::domains::review::api::response_api::json;
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::settings::user_config::ConfigUpdate;
use crate::domains::settings::user_config_wire::ConfigUpdateWire;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/config", get(show).put(update))
}

async fn show(State(state): State<ApiState>) -> Result<Response, ApiError> {
    json(&state.effective_config())
}

/// Stores the change, applies what a running review must follow at once, and answers with the
/// config now in force.
async fn update(
    State(state): State<ApiState>,
    JsonBody(wire): JsonBody<ConfigUpdateWire>,
) -> Result<Response, ApiError> {
    let update = ConfigUpdate::try_from(wire)?;
    state.config().set(&update).await?;
    if let Some(context) = update.context_lines {
        state.session().set_context(context).await?;
    }
    if let Some(follow_refs) = update.follow_refs {
        state.session().set_follow_refs(follow_refs);
    }
    state.session().announce(SessionEvent::Config);
    json(&state.effective_config())
}
