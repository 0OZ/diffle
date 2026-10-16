//! A commit's place on `HEAD`'s first-parent line, for stepping through history.

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::Exit;

/// The commits next to one along the first-parent line toward `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbours {
    /// The first parent; `None` for a root commit.
    pub parent: Option<ObjectId>,
    /// The commit whose first parent this is, toward `HEAD`. `None` at `HEAD`, on an unborn
    /// branch, or when the commit is off `HEAD`'s first-parent line (a merged side branch).
    pub child: Option<ObjectId>,
}

impl GitRepo {
    pub async fn commit_neighbours(&self, commit: &ObjectId) -> Result<Neighbours, GitError> {
        let parent_rev = RevName::of_object(commit).parent();
        let (parent, listed) = tokio::try_join!(
            self.resolve_commit(&parent_rev),
            self.descendants_toward_head(commit),
        )?;
        // Newest first, so the oldest descendant is last; it is the child only when it sits on
        // `commit` directly.
        let child = listed.lines().next_back().and_then(|line| {
            let mut ids = line.split(' ');
            let (child, first_parent) = (ids.next()?, ids.next()?);
            (first_parent == commit.as_str())
                .then(|| ObjectId::parse(child).ok())
                .flatten()
        });
        Ok(Neighbours { parent, child })
    }

    /// `rev-list --parents` of the first-parent ancestry path from `commit` to `HEAD`; an
    /// unborn `HEAD` or a failing walk lists nothing.
    async fn descendants_toward_head(&self, commit: &ObjectId) -> Result<String, GitError> {
        let range = format!("{commit}..HEAD");
        let finished = self
            .runner
            .finish([
                "rev-list",
                "--first-parent",
                "--ancestry-path",
                "--parents",
                &range,
                "--",
            ])
            .await?;
        Ok(match finished.exit() {
            Exit::Success => finished.into_stdout()?.text().into_owned(),
            Exit::Failure(_) | Exit::Terminated => String::new(),
        })
    }
}

#[cfg(test)]
#[path = "neighbours_tests.rs"]
mod tests;
