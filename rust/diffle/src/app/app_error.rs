//! The one error a run can end with, and the exit code each kind earns.

use std::io;
use std::process::ExitCode;

use thiserror::Error;

use crate::domains::repository::process::git_error::GitError;
use crate::domains::review::review_error::ReviewError;
use crate::domains::revisions::revspec_error::RevspecError;
use crate::domains::settings::config_store::ConfigError;
use crate::workers::http_server::server_error::ServerError;

/// A failure the program or the machine caused.
const EXIT_FAILURE: u8 = 1;
/// Input the reviewer can fix: bad revisions, an unknown revision, a directory that is not a
/// repository, a port that is taken.
const EXIT_USAGE: u8 = 2;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Revspec(#[from] RevspecError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Settings(#[from] ConfigError),
    #[error("cannot listen for Ctrl+C: {0}")]
    Signals(#[source] io::Error),
}

impl AppError {
    #[must_use]
    pub const fn exit_status(&self) -> u8 {
        match self {
            Self::Revspec(_) | Self::Git(_) => EXIT_USAGE,
            Self::Review(error) => review_status(error),
            Self::Server(error) => server_status(error),
            Self::Settings(_) | Self::Signals(_) => EXIT_FAILURE,
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from(self.exit_status())
    }
}

const fn review_status(error: &ReviewError) -> u8 {
    match error {
        ReviewError::Revspec(_)
        | ReviewError::Resolve(_)
        | ReviewError::Git(_)
        | ReviewError::NothingToFocus
        | ReviewError::NoUncommittedChanges { .. }
        | ReviewError::NotListed { .. } => EXIT_USAGE,
        ReviewError::Comments(_) | ReviewError::NotOpen | ReviewError::Interrupted => EXIT_FAILURE,
    }
}

const fn server_status(error: &ServerError) -> u8 {
    match error {
        ServerError::Bind { .. }
        | ServerError::PortTaken { .. }
        | ServerError::NoFreePort { .. } => EXIT_USAGE,
        ServerError::Serve(_) | ServerError::NoClientBuild { .. } => EXIT_FAILURE,
    }
}

#[cfg(test)]
#[path = "app_error_tests.rs"]
mod tests;
