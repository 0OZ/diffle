//! `GET /api/image`: one side of a file as picture bytes, under the key that names them.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;

use crate::domains::repository::contents::image_type::image_type;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::ApiQuery;
use crate::domains::review::api::wire::side_wire::SideWire;

const CACHE_FOREVER: &str = "private, max-age=31536000, immutable";
const NO_SNIFFING: &str = "nosniff";
/// An `<img>` from another site sends no `Origin`, so the origin guard lets it through; this
/// header makes the browser refuse the embed.
const SAME_ORIGIN_ONLY: &str = "same-origin";
const CROSS_ORIGIN_RESOURCE_POLICY: HeaderName =
    HeaderName::from_static("cross-origin-resource-policy");

#[derive(Debug, Deserialize)]
struct ImageQuery {
    path: String,
    rev: SideWire,
    /// The `imageKey` of that side in the current snapshot.
    key: String,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/image", get(image))
}

async fn image(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<ImageQuery>,
) -> Result<Response, ApiError> {
    if query.path.is_empty() || query.key.is_empty() {
        return Err(ApiError::bad_request("path and key are required"));
    }
    let (_, snapshot) = state.current().await?;
    let bytes = match RepoPath::parse(&query.path) {
        Ok(path) => {
            state
                .session()
                .read_image(&snapshot, &path, query.rev.into(), &query.key)
                .await?
        }
        Err(_) => None,
    };
    let Some((bytes, kind)) = bytes.and_then(|bytes| image_type(&bytes).map(|kind| (bytes, kind)))
    else {
        return Err(ApiError::NotFound("no image under that key".to_owned()));
    };
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static(kind.media_type()),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(CACHE_FOREVER),
            ),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static(NO_SNIFFING),
            ),
            (
                CROSS_ORIGIN_RESOURCE_POLICY,
                HeaderValue::from_static(SAME_ORIGIN_ONLY),
            ),
        ],
        bytes,
    )
        .into_response())
}
