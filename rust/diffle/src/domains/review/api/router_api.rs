//! The route table: every domain's routes, behind the gate that waits for the first review.

use axum::Router;
use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::any;

use super::api_state::ApiState;
use super::contents::{file_api, image_api, patch_api, search_api};
use super::error_api::ApiError;
use super::settings::config_api;
use super::snapshots::{history_api, mode_api, snapshot_api};
use super::threads::{thread_edits_api, threads_api, viewed_api};
use super::unsupported_api;

/// The config is the one thing the client can ask before a review exists.
const CONFIG_ROUTE: &str = "/api/config";
const UNKNOWN_ROUTE: &str = "API endpoint not found. Restart diffle to load the latest server.";

/// The API with its state applied, ready to be mounted.
pub fn router(state: ApiState) -> Router {
    Router::new()
        .merge(snapshot_api::routes())
        .merge(mode_api::routes())
        .merge(history_api::routes())
        .merge(patch_api::routes())
        .merge(file_api::routes())
        .merge(image_api::routes())
        .merge(search_api::routes())
        .merge(threads_api::routes())
        .merge(thread_edits_api::routes())
        .merge(viewed_api::routes())
        .merge(config_api::routes())
        .merge(unsupported_api::routes())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            wait_until_open,
        ))
        // After the gate: a path that is no route answers at once, ready or not. Paths outside
        // `/api` are not this router's, so the host serves the page for them.
        .route("/api/{*path}", any(unknown))
        .with_state(state)
}

/// Holds a request until the first review is open: the server listens before it has computed
/// one, and the page loads while that happens.
async fn wait_until_open(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    if request.uri().path() != CONFIG_ROUTE
        && let Err(error) = state.until_open().await
    {
        return error.into_response();
    }
    next.run(request).await
}

async fn unknown() -> Response {
    ApiError::NotFound(UNKNOWN_ROUTE.to_owned()).into_response()
}
