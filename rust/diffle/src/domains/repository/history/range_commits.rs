//! The commits a comparison spans.

use super::commit_info::CommitInfo;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::process::git_error::GitError;

/// The six fields of a log record: hash, abbreviation, author, email, time, message.
const FIELDS_PER_COMMIT: usize = 6;
const MILLIS_PER_SECOND: i64 = 1000;

/// Which commits to list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitRange {
    /// What `to` has that `from` lacks (`from..to`).
    Span { from: ObjectId, to: ObjectId },
    /// One commit alone (`tip^!`).
    Single(ObjectId),
}

impl CommitRange {
    fn revision(&self) -> String {
        match self {
            Self::Span { from, to } => format!("{from}..{to}"),
            Self::Single(tip) => format!("{tip}^!"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeCommit {
    pub info: CommitInfo,
    pub author: String,
    pub email: String,
    /// Author date, epoch milliseconds.
    pub date_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RangeCommits {
    /// Oldest first; the newest `limit` when there are more.
    pub list: Vec<RangeCommit>,
    pub total: u64,
}

impl GitRepo {
    pub async fn range_commits(
        &self,
        range: &CommitRange,
        limit: usize,
    ) -> Result<RangeCommits, GitError> {
        let revision = range.revision();
        let max_count = format!("--max-count={limit}");
        let (count, log) = tokio::try_join!(
            self.runner
                .stdout(["rev-list", "--count", "--end-of-options", &revision, "--"]),
            self.runner.stdout([
                "log",
                "-z",
                "--no-show-signature",
                "--reverse",
                &max_count,
                "--format=%H%x00%h%x00%an%x00%ae%x00%at%x00%B",
                "--end-of-options",
                &revision,
                "--",
            ]),
        )?;
        let total = count.trimmed().parse().map_err(|_| GitError::Unexpected {
            expected: "a commit count",
            output: count.trimmed(),
        })?;
        // Fields cannot contain NUL, and `-z` ends each record with one, so every commit is
        // exactly six fields.
        let fields = log.nul_records();
        let list = fields
            .chunks_exact(FIELDS_PER_COMMIT)
            .map(range_commit)
            .collect::<Result<_, _>>()?;
        Ok(RangeCommits { list, total })
    }
}

fn range_commit(fields: &[String]) -> Result<RangeCommit, GitError> {
    let [sha, short, author, email, time, message] = fields else {
        return Err(unexpected("six log fields", fields.join("\0")));
    };
    let id = ObjectId::parse(sha).map_err(|_| unexpected("an object id", sha.clone()))?;
    let seconds: i64 = time
        .parse()
        .map_err(|_| unexpected("an author time", time.clone()))?;
    Ok(RangeCommit {
        info: CommitInfo {
            id,
            short: short.clone(),
            message: message.trim_end().to_owned(),
        },
        author: author.clone(),
        email: email.clone(),
        date_ms: seconds.saturating_mul(MILLIS_PER_SECOND),
    })
}

fn unexpected(expected: &'static str, output: String) -> GitError {
    GitError::Unexpected { expected, output }
}

#[cfg(test)]
#[path = "range_commits_tests.rs"]
mod tests;
