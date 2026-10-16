//! `GET /api/patch` (one file, or all) and `POST /api/patch` (a bounded batch).

use axum::Router;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;

use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::{ApiQuery, JsonBody};
use crate::domains::review::api::response_api::text;

/// A client loads a review in bounded batches; more paths than this is not one.
const MAX_BATCH_PATHS: usize = 200;

#[derive(Debug, Deserialize)]
struct PatchQuery {
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PatchBatch {
    paths: Vec<String>,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/patch", get(one_or_all).post(many))
}

async fn one_or_all(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<PatchQuery>,
) -> Result<Response, ApiError> {
    let active = state.session().active()?;
    let Some(path) = query.path.filter(|path| !path.is_empty()) else {
        return Ok(text(active.snapshotter().patch_all().await?.as_str()));
    };
    let patch = match RepoPath::parse(&path) {
        Ok(path) => active.snapshotter().patch(&path).await?,
        Err(_) => None,
    };
    match patch {
        Some(patch) => Ok(text(patch.as_str())),
        None => Err(ApiError::NotFound("not a changed file".to_owned())),
    }
}

/// The patches of the changed files among `paths`, concatenated; unknown paths are skipped.
async fn many(
    State(state): State<ApiState>,
    JsonBody(batch): JsonBody<PatchBatch>,
) -> Result<Response, ApiError> {
    if batch.paths.len() > MAX_BATCH_PATHS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_BATCH_PATHS} paths per request"
        )));
    }
    let paths: Vec<RepoPath> = batch
        .paths
        .iter()
        .filter_map(|path| RepoPath::parse(path).ok())
        .collect();
    let active = state.session().active()?;
    Ok(text(active.snapshotter().patch_many(&paths).await?).into_response())
}
