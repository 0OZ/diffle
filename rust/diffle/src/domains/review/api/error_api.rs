//! What a failed request answers: a status, and `{"error": message}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

use crate::domains::comments::anchors::review_path::EmptyPath;
use crate::domains::comments::comment_error::{NotFound, Unquotable};
use crate::domains::comments::imports::import_wire::ImportError;
use crate::domains::comments::storage::store_error::CommentError;
use crate::domains::comments::threads::body::BodyError;
use crate::domains::repository::names::repo_path::RepoPathError;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::review::review_error::ReviewError;
use crate::domains::settings::config_store::ConfigError;
use crate::domains::settings::user_config_wire::ConfigUpdateError;

#[derive(Debug, Error)]
pub enum ApiError {
    /// The caller's input is wrong: the fix is theirs.
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    /// The request is fine but the server cannot honour it in its present state.
    #[error("{0}")]
    Conflict(String),
    #[error("request body too large")]
    TooLarge,
    /// A feature this server does not have yet.
    #[error("{0}")]
    NotImplemented(String),
    /// The server's own failure.
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    #[must_use]
    pub fn not_implemented(feature: &str) -> Self {
        Self::NotImplemented(format!("{feature} is not available in this server yet"))
    }

    #[must_use]
    pub const fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn internal(error: &impl std::error::Error) -> Self {
        tracing::error!(%error, "request failed");
        Self::Internal(error.to_string())
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.to_string(),
        };
        (self.status(), Json(body)).into_response()
    }
}

impl From<GitError> for ApiError {
    /// Git refusing a revision or path is the caller's; losing git itself is ours.
    fn from(error: GitError) -> Self {
        match error {
            GitError::Spawn(_) | GitError::Stream { .. } | GitError::Pipe(_) => {
                Self::internal(&error)
            }
            GitError::Failed { .. }
            | GitError::TimedOut { .. }
            | GitError::Read { .. }
            | GitError::BadPath(_)
            | GitError::NotADirectory { .. }
            | GitError::NotARepository { .. }
            | GitError::Unexpected { .. } => Self::BadRequest(error.to_string()),
        }
    }
}

impl From<ReviewError> for ApiError {
    fn from(error: ReviewError) -> Self {
        match error {
            ReviewError::Git(error) => error.into(),
            ReviewError::Comments(error) => error.into(),
            ReviewError::Revspec(_)
            | ReviewError::Resolve(_)
            | ReviewError::NothingToFocus
            | ReviewError::NoUncommittedChanges { .. }
            | ReviewError::NotListed { .. } => Self::BadRequest(error.to_string()),
            ReviewError::NotOpen | ReviewError::Interrupted => Self::internal(&error),
        }
    }
}

impl From<CommentError> for ApiError {
    fn from(error: CommentError) -> Self {
        match error {
            CommentError::NotFound(NotFound::Thread(_) | NotFound::Message(_)) => {
                Self::NotFound("not found".to_owned())
            }
            CommentError::Unquotable(Unquotable::MissingFile(_) | Unquotable::Range { .. }) => {
                Self::BadRequest(error.to_string())
            }
            CommentError::Lock(_)
            | CommentError::Io { .. }
            | CommentError::Malformed { .. }
            | CommentError::Invalid { .. } => Self::internal(&error),
        }
    }
}

impl From<ImportError> for ApiError {
    fn from(error: ImportError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<BodyError> for ApiError {
    fn from(error: BodyError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<EmptyPath> for ApiError {
    fn from(error: EmptyPath) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<RepoPathError> for ApiError {
    fn from(error: RepoPathError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<ConfigUpdateError> for ApiError {
    fn from(error: ConfigUpdateError) -> Self {
        Self::BadRequest(error.to_string())
    }
}

impl From<ConfigError> for ApiError {
    fn from(error: ConfigError) -> Self {
        Self::internal(&error)
    }
}

#[cfg(test)]
#[path = "error_api_tests.rs"]
mod tests;
