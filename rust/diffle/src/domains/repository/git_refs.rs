//! Branches, the upstream a branch tracks, and the configured remotes.

use std::fmt;

use super::git_repo::GitRepo;
use super::names::rev_name::RevName;
use super::process::git_error::GitError;
use super::process::git_runner::Exit;

const LOCAL_PREFIX: &str = "refs/heads/";
const REMOTE_PREFIX: &str = "refs/remotes/";
/// `%(upstream:remotename)` of a branch that tracks another local branch.
const LOCAL_REMOTE: &str = ".";

/// An exact branch, local or remote-tracking. Expressions, `worktree`, and a detached
/// `HEAD` name none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchRef {
    /// `refs/heads/<name>`.
    Local(String),
    /// `refs/remotes/<remote>/<name>`, held without the `refs/remotes/` prefix.
    Remote(String),
}

impl BranchRef {
    /// A full ref name; any ref outside `refs/heads/` and `refs/remotes/` is not a branch.
    pub fn parse(full_name: &str) -> Option<Self> {
        match (
            full_name.strip_prefix(LOCAL_PREFIX),
            full_name.strip_prefix(REMOTE_PREFIX),
        ) {
            (Some(name), _) => Some(Self::Local(name.to_owned())),
            (None, Some(name)) => Some(Self::Remote(name.to_owned())),
            (None, None) => None,
        }
    }

    pub fn full_name(&self) -> String {
        match self {
            Self::Local(name) => format!("{LOCAL_PREFIX}{name}"),
            Self::Remote(name) => format!("{REMOTE_PREFIX}{name}"),
        }
    }
}

impl fmt::Display for BranchRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_name())
    }
}

/// A configured remote and the URL it was configured with, before `insteadOf` rewriting
/// (which can hide the repository's identity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

/// The branch on a remote that a branch follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub remote: String,
    pub branch: String,
}

impl GitRepo {
    /// The branch `rev` names exactly, or `None` for anything else.
    pub async fn branch_ref(&self, rev: &RevName) -> Result<Option<BranchRef>, GitError> {
        let args: Vec<&str> = if rev.is_head() {
            vec!["symbolic-ref", "--quiet", "HEAD"]
        } else {
            vec![
                "rev-parse",
                "--symbolic-full-name",
                "--verify",
                "--end-of-options",
                rev.as_str(),
            ]
        };
        let finished = self.runner.finish(args).await?;
        match finished.exit() {
            Exit::Success => Ok(BranchRef::parse(&finished.into_stdout()?.trimmed())),
            Exit::Failure(_) | Exit::Terminated => Ok(None),
        }
    }

    /// The remote branch `branch` tracks. A remote-tracking branch is its own upstream.
    pub async fn upstream_branch(
        &self,
        branch: &BranchRef,
        remotes: &[Remote],
    ) -> Result<Option<Upstream>, GitError> {
        match branch {
            BranchRef::Local(_) => {
                let stdout = self
                    .runner
                    .stdout([
                        "for-each-ref",
                        "--format=%(upstream:remotename)%00%(upstream:remoteref)%00",
                        &branch.full_name(),
                    ])
                    .await?;
                let text = stdout.text();
                let mut fields = text.split('\0');
                let (Some(remote), Some(upstream)) = (fields.next(), fields.next()) else {
                    return Ok(None);
                };
                if remote.is_empty() || remote == LOCAL_REMOTE {
                    return Ok(None);
                }
                Ok(upstream.strip_prefix(LOCAL_PREFIX).map(|name| Upstream {
                    remote: remote.to_owned(),
                    branch: name.to_owned(),
                }))
            }
            BranchRef::Remote(tail) => {
                // Remote names can contain slashes; the longest configured name wins.
                let mut by_length: Vec<&Remote> = remotes.iter().collect();
                by_length.sort_by_key(|remote| std::cmp::Reverse(remote.name.len()));
                Ok(by_length.into_iter().find_map(|remote| {
                    tail.strip_prefix(&format!("{}/", remote.name))
                        .map(|name| Upstream {
                            remote: remote.name.clone(),
                            branch: name.to_owned(),
                        })
                }))
            }
        }
    }

    /// Configured remotes in Git's order.
    pub async fn remotes(&self) -> Result<Vec<Remote>, GitError> {
        let finished = self
            .runner
            .finish(["config", "--null", "--get-regexp", "^remote\\..*\\.url$"])
            .await?;
        let stdout = match finished.exit() {
            Exit::NO_ANSWER => return Ok(Vec::new()),
            Exit::Success | Exit::Failure(_) | Exit::Terminated => finished.into_stdout()?,
        };
        let mut remotes: Vec<Remote> = Vec::new();
        for record in stdout.text().split('\0') {
            let Some((key, url)) = record.split_once('\n') else {
                continue;
            };
            let Some(name) = key
                .strip_prefix("remote.")
                .and_then(|key| key.strip_suffix(".url"))
            else {
                continue;
            };
            if remotes.iter().all(|remote| remote.name != name) {
                remotes.push(Remote {
                    name: name.to_owned(),
                    url: url.to_owned(),
                });
            }
        }
        Ok(remotes)
    }
}

#[cfg(test)]
#[path = "git_refs_tests.rs"]
mod tests;
