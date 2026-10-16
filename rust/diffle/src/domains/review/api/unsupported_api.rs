//! Routes of features this server does not have yet. The client already calls them, so each
//! answers the way a server without the feature would: a status it handles, and the reason.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::{delete, get, post};
use serde::Serialize;

use super::api_state::ApiState;
use super::error_api::ApiError;
use super::response_api::json;

const LSP_OFF: &str = "language servers are not available in this server yet";
const GITHUB_OFF: &str = "GitHub is not available in this server yet";

/// `GithubMetadataSchema` with nothing known.
#[derive(Debug, Serialize)]
struct GithubMetadataWire {
    version: u64,
    repository: Option<String>,
    #[serde(rename = "pullRequest")]
    pull_request: Option<()>,
    reason: &'static str,
}

/// `LspStatusSchema` with no servers and none missing.
#[derive(Debug, Serialize)]
struct LspStatusWire {
    enabled: bool,
    servers: [(); 0],
    missing: [(); 0],
}

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/github", get(github))
        .route("/api/github/export", post(github_export))
        .route("/api/lsp/status", get(lsp_status))
        .route("/api/lsp/symbols", get(lsp_unavailable))
        .route("/api/lsp/definition", post(lsp_unavailable))
        .route("/api/lsp/type-definition", post(lsp_unavailable))
        .route("/api/lsp/hover", post(lsp_unavailable))
        .route("/api/lsp/token-kind", post(lsp_unavailable))
        .route("/api/lsp/references", post(lsp_unavailable))
        .route("/api/lsp/occurrences", post(lsp_unavailable))
        .route("/api/iterations", delete(iterations))
        .route("/api/iterations/{index}", delete(iterations))
}

async fn github(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let (_, snapshot) = state.current().await?;
    json(&GithubMetadataWire {
        version: snapshot.version().get(),
        repository: None,
        pull_request: None,
        reason: GITHUB_OFF,
    })
}

async fn github_export() -> Result<Response, ApiError> {
    Err(ApiError::not_implemented("exporting to GitHub"))
}

async fn lsp_status() -> Result<Response, ApiError> {
    json(&LspStatusWire {
        enabled: false,
        servers: [],
        missing: [],
    })
}

async fn lsp_unavailable() -> Result<Response, ApiError> {
    Err(ApiError::Conflict(LSP_OFF.to_owned()))
}

async fn iterations() -> Result<Response, ApiError> {
    Err(ApiError::not_implemented("recorded iterations"))
}
