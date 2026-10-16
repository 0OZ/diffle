//! What can go wrong talking to Git.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use super::git_runner::Exit;
use crate::domains::repository::names::repo_path::RepoPathError;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("cannot run git: {0}")]
    Spawn(#[source] io::Error),
    #[error("git {command} failed ({exit}): {stderr}")]
    Failed {
        command: String,
        exit: Exit,
        stderr: String,
    },
    #[error("git {command} ran longer than {after:?} and was stopped")]
    TimedOut { command: String, after: Duration },
    #[error("lost the output of git {command}: {source}")]
    Stream {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("lost the pipe to git cat-file --batch: {0}")]
    Pipe(#[source] io::Error),
    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("git named a path outside the repository: {0}")]
    BadPath(#[from] RepoPathError),
    #[error("not a directory: {}", dir.display())]
    NotADirectory { dir: PathBuf },
    #[error("not a git repository: {}", dir.display())]
    NotARepository { dir: PathBuf },
    #[error("expected {expected} from git, got {output:?}")]
    Unexpected {
        expected: &'static str,
        output: String,
    },
}
