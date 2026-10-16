//! An open Git repository: where it lives, and the questions about commits that every
//! review starts from.

use std::path::{Path, PathBuf};

use super::names::object_id::ObjectId;
use super::names::rev_name::RevName;
use super::process::cat_file_batch::ObjectReader;
use super::process::git_error::GitError;
use super::process::git_runner::{Exit, GitRunner, Stdout};

#[derive(Debug, Clone)]
pub struct GitRepo {
    root: PathBuf,
    git_dir: PathBuf,
    /// Shared by every worktree (refs, packed-refs); equals `git_dir` for the main one.
    common_dir: PathBuf,
    pub(super) runner: GitRunner,
    pub(super) objects: ObjectReader,
}

impl GitRepo {
    /// The repository containing `dir`. A `dir` that is not a directory is
    /// [`GitError::NotADirectory`]; anything Git cannot place inside a worktree is
    /// [`GitError::NotARepository`].
    pub async fn open(dir: &Path) -> Result<Self, GitError> {
        // A missing working directory fails the spawn with the same `ENOENT` as a missing `git`.
        if !tokio::fs::metadata(dir)
            .await
            .is_ok_and(|meta| meta.is_dir())
        {
            return Err(GitError::NotADirectory {
                dir: dir.to_owned(),
            });
        }
        let finished = GitRunner::new(dir)
            .finish([
                "rev-parse",
                // Without this a path is relative to `dir`, which may be below the root.
                "--path-format=absolute",
                "--show-toplevel",
                "--git-dir",
                "--git-common-dir",
            ])
            .await?;
        let stdout = match finished.exit() {
            Exit::Success => finished.into_stdout()?,
            Exit::Failure(_) | Exit::Terminated => {
                return Err(GitError::NotARepository {
                    dir: dir.to_owned(),
                });
            }
        };
        let text = stdout.text();
        let mut lines = text.lines();
        let (Some(root), Some(git_dir)) = (lines.next(), lines.next()) else {
            return Err(GitError::Unexpected {
                expected: "a worktree root and a git dir",
                output: text.into_owned(),
            });
        };
        let root = PathBuf::from(root);
        let git_dir = PathBuf::from(git_dir);
        let common_dir = lines.next().map_or_else(|| git_dir.clone(), PathBuf::from);
        Ok(Self {
            runner: GitRunner::new(&root),
            objects: ObjectReader::new(&root),
            git_dir,
            common_dir,
            root,
        })
    }

    /// How many `cat-file --batch` processes this repository has started. Diagnostics and tests.
    #[must_use]
    pub fn objects_spawned(&self) -> usize {
        self.objects.spawns()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn common_dir(&self) -> &Path {
        &self.common_dir
    }

    /// The commit `rev` names, or `None` when it names no commit (an unknown name, or `HEAD`
    /// of a repository without commits). `--end-of-options` keeps an option-shaped name a name.
    pub async fn resolve_commit(&self, rev: &RevName) -> Result<Option<ObjectId>, GitError> {
        let peeled = format!("{rev}^{{commit}}");
        let finished = self
            .runner
            .finish([
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &peeled,
            ])
            .await?;
        match finished.exit() {
            Exit::NO_ANSWER => Ok(None),
            Exit::Success | Exit::Failure(_) | Exit::Terminated => {
                object_id(&finished.into_stdout()?).map(Some)
            }
        }
    }

    /// The empty tree under the repository's hash algorithm: the old side of an unborn branch.
    pub async fn empty_tree(&self) -> Result<ObjectId, GitError> {
        object_id(
            &self
                .runner
                .stdout(["hash-object", "-t", "tree", "--stdin"])
                .await?,
        )
    }

    /// The best common ancestor of two commits, or `None` when their histories are unrelated.
    pub async fn merge_base(
        &self,
        a: &ObjectId,
        b: &ObjectId,
    ) -> Result<Option<ObjectId>, GitError> {
        let finished = self
            .runner
            .finish(["merge-base", "--end-of-options", a.as_str(), b.as_str()])
            .await?;
        match finished.exit() {
            Exit::NO_ANSWER => Ok(None),
            Exit::Success | Exit::Failure(_) | Exit::Terminated => {
                object_id(&finished.into_stdout()?).map(Some)
            }
        }
    }
}

fn object_id(stdout: &Stdout) -> Result<ObjectId, GitError> {
    let text = stdout.trimmed();
    ObjectId::parse(&text).map_err(|_| GitError::Unexpected {
        expected: "an object id",
        output: text,
    })
}

#[cfg(test)]
#[path = "git_repo_tests.rs"]
mod tests;
