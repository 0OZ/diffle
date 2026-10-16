//! One commit as the review shows it.

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::process::git_error::GitError;

const FIELD_SEPARATOR: char = '\0';

/// A commit's hash, abbreviation and full message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: ObjectId,
    pub short: String,
    pub message: String,
}

impl GitRepo {
    /// The commit `rev` names, or `None` when it names none.
    pub async fn commit_info(&self, rev: &RevName) -> Result<Option<CommitInfo>, GitError> {
        let Some(id) = self.resolve_commit(rev).await? else {
            return Ok(None);
        };
        let output = self
            .runner
            .stdout([
                "log",
                "-1",
                "--no-show-signature",
                "--format=%h%x00%B",
                id.as_str(),
                "--",
            ])
            .await?
            .text()
            .into_owned();
        let Some((short, message)) = output.split_once(FIELD_SEPARATOR) else {
            return Err(GitError::Unexpected {
                expected: "an abbreviation and a message",
                output,
            });
        };
        Ok(Some(CommitInfo {
            id,
            short: short.to_owned(),
            message: message.trim_end().to_owned(),
        }))
    }

    /// The commit `offset` first-parent steps behind `HEAD` (`HEAD` itself at 0).
    pub async fn commit_behind_head(&self, offset: u32) -> Result<Option<CommitInfo>, GitError> {
        let rev = match offset {
            0 => RevName::head(),
            behind => RevName::head().ancestor(behind),
        };
        self.commit_info(&rev).await
    }
}

#[cfg(test)]
#[path = "commit_info_tests.rs"]
mod tests;
