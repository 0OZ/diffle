//! `GET /api/file`: one side of a file of the review.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

use crate::domains::repository::contents::binary::is_binary;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::ApiQuery;
use crate::domains::review::api::response_api::json;
use crate::domains::review::api::wire::side_wire::SideWire;

#[derive(Debug, Deserialize)]
struct FileQuery {
    path: String,
    rev: SideWire,
}

/// `FileResponseSchema`: a binary file has no text to show.
#[derive(Debug, Serialize)]
struct FileWire {
    path: String,
    contents: String,
    binary: bool,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/file", get(file))
}

async fn file(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<FileQuery>,
) -> Result<Response, ApiError> {
    if query.path.is_empty() {
        return Err(ApiError::bad_request("path is required"));
    }
    let (_, snapshot) = state.current().await?;
    // A path the snapshot does not expose, or that is no path at all, is simply absent.
    let bytes = match RepoPath::parse(&query.path) {
        Ok(path) => {
            state
                .session()
                .read_side(&snapshot, &path, query.rev.into())
                .await?
        }
        Err(_) => None,
    };
    let Some(bytes) = bytes else {
        return Err(ApiError::NotFound("absent on that side".to_owned()));
    };
    let binary = is_binary(&bytes);
    json(&FileWire {
        path: query.path,
        contents: if binary {
            String::new()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        },
        binary,
    })
}
