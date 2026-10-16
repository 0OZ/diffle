//! `GET /api/search`: find text in the review.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Deserializer, Serialize};

use super::search_plan::{SearchContent, SearchScope, files_in_scope, shown_lines};
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::search::grep::{GrepOptions, SearchHits};
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::ApiQuery;
use crate::domains::review::api::response_api::json;

/// More matches than a reviewer reads; the response says when it stopped.
const MATCH_LIMIT: usize = 500;
/// The query string spells a switch `1`; anything else, or nothing, is off.
const FLAG_ON: &str = "1";

#[derive(Debug, Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    scope: SearchScope,
    #[serde(default)]
    content: SearchContent,
    path: Option<String>,
    #[serde(default, deserialize_with = "flag")]
    word: bool,
    #[serde(default, deserialize_with = "flag")]
    i: bool,
    #[serde(default, deserialize_with = "flag")]
    re: bool,
}

fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    String::deserialize(deserializer).map(|text| text == FLAG_ON)
}

/// `SearchResponseSchema`.
#[derive(Debug, Serialize)]
struct SearchWire<'a> {
    query: &'a str,
    matches: Vec<MatchWire<'a>>,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct MatchWire<'a> {
    path: &'a str,
    /// 1-based line on the new side.
    line: u64,
    text: &'a str,
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/search", get(search))
}

async fn search(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<SearchQuery>,
) -> Result<Response, ApiError> {
    let (active, snapshot) = state.current().await?;
    // A file scope names a path; one that is no path matches nothing.
    let path = query
        .path
        .as_deref()
        .and_then(|path| RepoPath::parse(path).ok());
    let mut paths = files_in_scope(&snapshot, query.scope, path.as_ref());
    let ranges = match query.content {
        SearchContent::Full => None,
        SearchContent::Diff => {
            let (shown, ranges) =
                shown_lines(active.snapshotter(), &snapshot, paths.as_deref()).await?;
            paths = Some(shown);
            Some(ranges)
        }
    };
    let options = GrepOptions {
        word: query.word,
        ignore_case: query.i,
        regex: query.re,
        paths,
        ranges,
    };
    let SearchHits { matches, truncated } = state
        .session()
        .repo()
        .grep(&query.q, &snapshot.parts().new, MATCH_LIMIT, &options)
        .await?;
    json(&SearchWire {
        query: &query.q,
        matches: matches
            .iter()
            .map(|found| MatchWire {
                path: found.path.as_str(),
                line: found.line,
                text: &found.text,
            })
            .collect(),
        truncated,
    })
}
