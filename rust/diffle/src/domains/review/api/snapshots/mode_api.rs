//! `POST /api/mode`: show something else.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use serde::Deserialize;

use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::error_api::ApiError;
use crate::domains::review::api::input_api::JsonBody;
use crate::domains::review::api::response_api::json;
use crate::domains::review::api::wire::snapshot_wire::SnapshotWire;
use crate::domains::review::mode::mode_request::{FocusTarget, ModeRequest};
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;

/// What `focus` calls the uncommitted changes of the range.
const WORKTREE: &str = "worktree";

/// `ModeRequestSchema`. The kinds this server cannot open are still parsed, so a client that
/// asks for one hears what is missing instead of a complaint about its JSON.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ModeRequestWire {
    Working,
    Pr {},
    Revspec {
        args: Vec<String>,
    },
    /// A listed commit, `worktree`, or with `null` the range itself.
    Focus {
        commit: Option<String>,
    },
    Interdiff {},
    RangeDiff {},
    Pair {},
}

pub fn routes() -> Router<ApiState> {
    Router::new().route("/api/mode", post(switch))
}

impl TryFrom<ModeRequestWire> for ModeRequest {
    type Error = ApiError;

    fn try_from(wire: ModeRequestWire) -> Result<Self, Self::Error> {
        match wire {
            ModeRequestWire::Working => Ok(Self::Enter(EntryRequest::Working)),
            ModeRequestWire::Revspec { args } => {
                let spec = Revspec::parse(&args)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                Ok(Self::Enter(EntryRequest::Revspec(spec)))
            }
            ModeRequestWire::Focus { commit } => Ok(Self::Focus(match commit.as_deref() {
                None => FocusTarget::Range,
                Some(WORKTREE) => FocusTarget::Worktree,
                Some(commit) => FocusTarget::Commit(commit.to_owned()),
            })),
            ModeRequestWire::Pr {} => Err(ApiError::not_implemented("reviewing a pull request")),
            ModeRequestWire::Interdiff {} => Err(ApiError::not_implemented("an interdiff")),
            ModeRequestWire::RangeDiff {} => Err(ApiError::not_implemented("a range-diff")),
            ModeRequestWire::Pair {} => Err(ApiError::not_implemented("an interdiff pair")),
        }
    }
}

async fn switch(
    State(state): State<ApiState>,
    JsonBody(wire): JsonBody<ModeRequestWire>,
) -> Result<Response, ApiError> {
    let snapshot = state.session().switch_mode(wire.try_into()?).await?;
    json(&SnapshotWire::from(&*snapshot))
}

#[cfg(test)]
#[path = "mode_api_tests.rs"]
mod tests;
