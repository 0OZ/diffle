//! `GET /api/refs` and `GET /api/last-commits-preview`: what the picker chooses from.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use serde::{Deserialize, Serialize};

use crate::domains::repository::history::commit_info::CommitInfo;
use crate::domains::repository::history::refs_listing::RefsListing;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::ApiQuery;
use crate::domains::review::api::response_api::json;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/api/refs", get(refs))
        .route("/api/last-commits-preview", get(last_commits_preview))
}

/// `RefsResponseSchema`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefsWire<'a> {
    default_branch: Option<&'a str>,
    current: Option<&'a str>,
    branches: &'a [String],
    remote_branches: &'a [String],
    tags: &'a [String],
    recent: Vec<RecentCommitWire<'a>>,
}

#[derive(Debug, Serialize)]
struct RecentCommitWire<'a> {
    sha: &'a str,
    short: &'a str,
    subject: &'a str,
}

/// `CommitInfoSchema`.
#[derive(Debug, Serialize)]
struct CommitInfoWire<'a> {
    sha: &'a str,
    short: &'a str,
    message: &'a str,
}

/// `LastCommitsPreviewSchema`: `null` where the commit does not exist.
#[derive(Debug, Serialize)]
struct LastCommitsPreviewWire<'a> {
    old: Option<CommitInfoWire<'a>>,
    new: Option<CommitInfoWire<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LastCommitsQuery {
    old_offset: String,
    new_offset: String,
}

impl<'a> From<&'a RefsListing> for RefsWire<'a> {
    fn from(listing: &'a RefsListing) -> Self {
        Self {
            default_branch: listing.default_branch.as_deref(),
            current: listing.current.as_deref(),
            branches: &listing.branches,
            remote_branches: &listing.remote_branches,
            tags: &listing.tags,
            recent: listing
                .recent
                .iter()
                .map(|commit| RecentCommitWire {
                    sha: &commit.sha,
                    short: &commit.short,
                    subject: &commit.subject,
                })
                .collect(),
        }
    }
}

impl<'a> From<&'a CommitInfo> for CommitInfoWire<'a> {
    fn from(info: &'a CommitInfo) -> Self {
        Self {
            sha: info.id.as_str(),
            short: &info.short,
            message: &info.message,
        }
    }
}

async fn refs(State(state): State<ApiState>) -> Result<Response, ApiError> {
    let listing = state.session().repo().refs_listing().await?;
    json(&RefsWire::from(&listing))
}

async fn last_commits_preview(
    State(state): State<ApiState>,
    ApiQuery(query): ApiQuery<LastCommitsQuery>,
) -> Result<Response, ApiError> {
    let (old_offset, new_offset) = (offset(&query.old_offset)?, offset(&query.new_offset)?);
    let repo = state.session().repo();
    // Both ends are read against the one `HEAD` seen first, so a commit landing between them
    // cannot make the pair inconsistent.
    let Some(head) = repo.commit_info(&RevName::head()).await? else {
        return json(&LastCommitsPreviewWire {
            old: None,
            new: None,
        });
    };
    let behind = |offset: u32| {
        let head = head.clone();
        async move {
            match offset {
                0 => Ok(Some(head)),
                behind => {
                    repo.commit_info(&RevName::of_object(&head.id).ancestor(behind))
                        .await
                }
            }
        }
    };
    let (old, new) = tokio::try_join!(behind(old_offset), behind(new_offset))?;
    json(&LastCommitsPreviewWire {
        old: old.as_ref().map(Into::into),
        new: new.as_ref().map(Into::into),
    })
}

/// A non-negative whole number, written in digits only.
fn offset(text: &str) -> Result<u32, ApiError> {
    let digits = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    digits
        .then(|| text.parse().ok())
        .flatten()
        .ok_or_else(|| ApiError::bad_request(format!("not an offset: {text}")))
}

#[cfg(test)]
#[path = "history_api_tests.rs"]
mod tests;
