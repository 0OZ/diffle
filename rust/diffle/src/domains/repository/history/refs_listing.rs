//! Branches, tags and recent commits for the picker that chooses what to compare.

use super::commit_info::CommitInfo;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::Exit;

const RECENT_COMMITS: usize = 30;
const REMOTE_HEAD_SUFFIX: &str = "/HEAD";
const DETACHED: &str = "HEAD";
/// Where to look for the default branch when `origin/HEAD` is not set, best first.
const DEFAULT_BRANCH_CANDIDATES: [&str; 4] = ["origin/main", "origin/master", "main", "master"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentCommit {
    pub sha: String,
    pub short: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefsListing {
    pub default_branch: Option<String>,
    /// `None` when `HEAD` is detached or the branch is unborn.
    pub current: Option<String>,
    /// Newest commit first.
    pub branches: Vec<String>,
    pub remote_branches: Vec<String>,
    /// Newest first.
    pub tags: Vec<String>,
    pub recent: Vec<RecentCommit>,
}

impl GitRepo {
    pub async fn refs_listing(&self) -> Result<RefsListing, GitError> {
        let (branches, remotes, tags, recent, current, default_branch) = tokio::try_join!(
            self.short_refs("-committerdate", "refs/heads"),
            self.short_refs("-committerdate", "refs/remotes"),
            self.short_refs("-creatordate", "refs/tags"),
            self.recent_commits(),
            self.current_branch(),
            self.default_branch(),
        )?;
        Ok(RefsListing {
            default_branch,
            current,
            branches,
            // `origin/HEAD` shortens to `origin`; symbolic remote heads are not branches.
            remote_branches: remotes
                .into_iter()
                .filter(|name| name.contains('/') && !name.ends_with(REMOTE_HEAD_SUFFIX))
                .collect(),
            tags,
            recent,
        })
    }

    /// `origin/HEAD`, else the first of `origin/main`, `origin/master`, `main`, `master` that exists.
    pub async fn default_branch(&self) -> Result<Option<String>, GitError> {
        let finished = self
            .runner
            .finish([
                "symbolic-ref",
                "--quiet",
                "--short",
                "refs/remotes/origin/HEAD",
            ])
            .await?;
        if finished.exit() == Exit::Success {
            let name = finished.into_stdout()?.trimmed();
            if !name.is_empty() {
                return Ok(Some(name));
            }
        }
        for candidate in DEFAULT_BRANCH_CANDIDATES {
            let Ok(rev) = RevName::parse(candidate) else {
                continue;
            };
            if self.resolve_commit(&rev).await?.is_some() {
                return Ok(Some(candidate.to_owned()));
            }
        }
        Ok(None)
    }

    async fn short_refs(&self, sort: &str, namespace: &str) -> Result<Vec<String>, GitError> {
        let stdout = self
            .runner
            .stdout([
                "for-each-ref",
                "--format=%(refname:short)",
                &format!("--sort={sort}"),
                namespace,
            ])
            .await?;
        Ok(non_empty_lines(&stdout.text()))
    }

    /// An unborn branch has no log; the picker still needs the refs.
    async fn recent_commits(&self) -> Result<Vec<RecentCommit>, GitError> {
        let finished = self
            .runner
            .finish([
                "log",
                "-n",
                &RECENT_COMMITS.to_string(),
                "--format=%H%x00%h%x00%s",
            ])
            .await?;
        let text = match finished.exit() {
            Exit::Success => finished.into_stdout()?.text().into_owned(),
            Exit::Failure(_) | Exit::Terminated => String::new(),
        };
        Ok(non_empty_lines(&text)
            .into_iter()
            .filter_map(|line| {
                let mut fields = line.split('\0');
                Some(RecentCommit {
                    sha: fields.next()?.to_owned(),
                    short: fields.next()?.to_owned(),
                    subject: fields.next()?.to_owned(),
                })
            })
            .collect())
    }

    async fn current_branch(&self) -> Result<Option<String>, GitError> {
        let finished = self
            .runner
            .finish(["rev-parse", "--abbrev-ref", "HEAD"])
            .await?;
        Ok(match finished.exit() {
            Exit::Success => {
                Some(finished.into_stdout()?.trimmed()).filter(|name| name != DETACHED)
            }
            Exit::Failure(_) | Exit::Terminated => None,
        })
    }
}

impl From<CommitInfo> for RecentCommit {
    fn from(info: CommitInfo) -> Self {
        let subject = info.message.lines().next().unwrap_or_default().to_owned();
        Self {
            sha: info.id.to_string(),
            short: info.short,
            subject,
        }
    }
}

fn non_empty_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "refs_listing_tests.rs"]
mod tests;
