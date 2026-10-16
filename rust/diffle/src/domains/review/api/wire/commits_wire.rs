//! The commits a snapshot lists: `ReviewedCommitSchema` and `RangeCommitsSchema`.

use serde::Serialize;

use crate::domains::repository::history::range_commits::RangeCommit;
use crate::domains::review::snapshot::review_snapshot::{ReviewedCommit, SpannedCommits};

#[derive(Debug, Serialize)]
pub struct ReviewedCommitWire<'a> {
    sha: &'a str,
    short: &'a str,
    message: &'a str,
    /// First parent; `null` for a root commit.
    parent: Option<&'a str>,
    /// The commit whose first parent this is, toward `HEAD`.
    child: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct RangeCommitWire<'a> {
    sha: &'a str,
    short: &'a str,
    message: &'a str,
    author: &'a str,
    email: &'a str,
    /// Author date, epoch milliseconds.
    date: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeCommitsWire<'a> {
    list: Vec<RangeCommitWire<'a>>,
    total: u64,
    old_sha: String,
    new_sha: String,
}

impl<'a> From<&'a ReviewedCommit> for ReviewedCommitWire<'a> {
    fn from(commit: &'a ReviewedCommit) -> Self {
        Self {
            sha: commit.info.id.as_str(),
            short: &commit.info.short,
            message: &commit.info.message,
            parent: commit.neighbours.parent.as_ref().map(|id| id.as_str()),
            child: commit.neighbours.child.as_ref().map(|id| id.as_str()),
        }
    }
}

impl<'a> From<&'a RangeCommit> for RangeCommitWire<'a> {
    fn from(commit: &'a RangeCommit) -> Self {
        Self {
            sha: commit.info.id.as_str(),
            short: &commit.info.short,
            message: &commit.info.message,
            author: &commit.author,
            email: &commit.email,
            date: commit.date_ms,
        }
    }
}

impl<'a> From<&'a SpannedCommits> for RangeCommitsWire<'a> {
    fn from(spanned: &'a SpannedCommits) -> Self {
        Self {
            list: spanned.commits.list.iter().map(Into::into).collect(),
            total: spanned.commits.total,
            old_sha: spanned.old.to_string(),
            new_sha: spanned.new.to_string(),
        }
    }
}
