//! Where the built web client lives on disk.

use std::path::{Path, PathBuf};

use crate::workers::http_server::server_error::ServerError;

const INDEX: &str = "index.html";
/// Where a release or a checkout keeps the build, relative to the executable's directory and
/// then to the working directory.
const BESIDE_EXECUTABLE: [&str; 3] = ["client", "../client", "../../dist/client"];
const IN_WORKING_DIRECTORY: &str = "dist/client";

/// A directory known to hold a client build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientDir(PathBuf);

impl ClientDir {
    /// `explicit` when given, which must hold a build; otherwise the first usual place that does.
    pub fn locate(explicit: Option<&Path>) -> Result<Self, ServerError> {
        match explicit {
            Some(dir) => Self::at(dir),
            None => usual_places()
                .into_iter()
                .find_map(|dir| Self::at(&dir).ok())
                .ok_or_else(|| ServerError::NoClientBuild {
                    dir: PathBuf::from(IN_WORKING_DIRECTORY),
                }),
        }
    }

    fn at(dir: &Path) -> Result<Self, ServerError> {
        match dir.join(INDEX).is_file() {
            true => Ok(Self(dir.to_owned())),
            false => Err(ServerError::NoClientBuild {
                dir: dir.to_owned(),
            }),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    #[must_use]
    pub fn index(&self) -> PathBuf {
        self.0.join(INDEX)
    }
}

fn usual_places() -> Vec<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_owned))
        .into_iter()
        .flat_map(|dir| BESIDE_EXECUTABLE.map(|relative| dir.join(relative)));
    beside
        .chain(std::iter::once(PathBuf::from(IN_WORKING_DIRECTORY)))
        .collect()
}

#[cfg(test)]
#[path = "client_dir_tests.rs"]
mod tests;
