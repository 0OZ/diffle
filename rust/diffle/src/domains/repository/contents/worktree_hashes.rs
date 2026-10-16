//! What Git would store for the worktree's entries: the ids a snapshot gives the new side
//! of a comparison that ends at the checkout.

use std::path::Path;

use tokio::fs;

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::{Exit, GitRunner};
use crate::kernel::bounded::map_limit;

/// Concurrent filesystem probes and single-path `hash-object` calls.
const HASH_CONCURRENCY: usize = 8;

/// An entry of the worktree to hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeEntry {
    /// A regular file or a symlink.
    File(RepoPath),
    /// A checked-out submodule: its id is the commit it is at.
    Submodule(RepoPath),
}

/// How one entry gets its id.
enum Plan {
    Done(Option<ObjectId>),
    /// Rides the one `hash-object --stdin-paths` call.
    Batch,
    /// A path with a newline cannot ride a line-based list.
    Alone,
}

impl GitRepo {
    /// The id Git would give each entry, in order. An entry that cannot be hashed (gone since
    /// the listing, unreadable, a submodule that is not a repository) has none: one bad path
    /// never fails the rest.
    pub async fn hash_worktree(
        &self,
        entries: &[WorktreeEntry],
    ) -> Result<Vec<Option<ObjectId>>, GitError> {
        let indexes: Vec<usize> = (0..entries.len()).collect();
        let plans = map_limit(indexes, HASH_CONCURRENCY, |i| async move {
            match entries.get(i) {
                Some(entry) => self.plan(entry).await,
                None => Ok(Plan::Done(None)),
            }
        })
        .await;
        let mut hashes = vec![None; entries.len()];
        let (mut batch, mut alone) = (Vec::new(), Vec::new());
        for (i, plan) in plans.into_iter().enumerate() {
            match plan? {
                Plan::Done(id) => set(&mut hashes, i, id),
                Plan::Batch => batch.push(i),
                Plan::Alone => alone.push(i),
            }
        }
        if let Some(ids) = self.hash_listed(entries, &batch).await? {
            for (&i, id) in batch.iter().zip(ids) {
                set(&mut hashes, i, Some(id));
            }
        } else {
            // One unreadable path aborts Git's whole batch; retry each on its own.
            alone.extend(&batch);
        }
        let singles = map_limit(alone.clone(), HASH_CONCURRENCY, |i| async move {
            match entries.get(i) {
                Some(WorktreeEntry::File(path) | WorktreeEntry::Submodule(path)) => {
                    self.hash_one(path).await
                }
                None => Ok(None),
            }
        })
        .await;
        for (i, id) in alone.into_iter().zip(singles) {
            set(&mut hashes, i, id?);
        }
        Ok(hashes)
    }

    /// The id of `content` as the file at `path` would be stored, its clean filters applied.
    pub async fn hash_content(
        &self,
        content: &[u8],
        path: &RepoPath,
    ) -> Result<Option<ObjectId>, GitError> {
        let finished = self
            .runner
            .finish_with_input(
                ["hash-object", "--stdin", &format!("--path={path}")],
                content,
            )
            .await?;
        Ok(parse_id(finished.exit(), finished.into_stdout().ok()))
    }

    async fn plan(&self, entry: &WorktreeEntry) -> Result<Plan, GitError> {
        let path = match entry {
            WorktreeEntry::Submodule(path) => {
                return Ok(Plan::Done(self.submodule_head(path).await?));
            }
            WorktreeEntry::File(path) => path,
        };
        let file = self.root().join(path.as_str());
        let Ok(metadata) = fs::symlink_metadata(&file).await else {
            return Ok(Plan::Done(None));
        };
        match (metadata.is_symlink(), metadata.is_file()) {
            (true, _) => {
                let Ok(target) = fs::read_link(&file).await else {
                    return Ok(Plan::Done(None));
                };
                let target = target.as_os_str().as_encoded_bytes().to_vec();
                Ok(Plan::Done(self.hash_content(&target, path).await?))
            }
            (false, true) if path.as_str().contains('\n') => Ok(Plan::Alone),
            (false, true) => Ok(Plan::Batch),
            (false, false) => Ok(Plan::Done(None)),
        }
    }

    /// One `hash-object --stdin-paths`; `None` when Git fails or answers out of step.
    async fn hash_listed(
        &self,
        entries: &[WorktreeEntry],
        listed: &[usize],
    ) -> Result<Option<Vec<ObjectId>>, GitError> {
        if listed.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let mut input = Vec::new();
        for &i in listed {
            if let Some(WorktreeEntry::File(path) | WorktreeEntry::Submodule(path)) = entries.get(i)
            {
                input.extend_from_slice(path.as_str().as_bytes());
                input.push(b'\n');
            }
        }
        let finished = self
            .runner
            .finish_with_input(["hash-object", "--stdin-paths"], &input)
            .await?;
        let exit = finished.exit();
        let Ok(stdout) = finished.into_stdout() else {
            return Ok(None);
        };
        let ids: Vec<ObjectId> = stdout
            .text()
            .lines()
            .map_while(|line| ObjectId::parse(line.trim()).ok())
            .collect();
        Ok((exit == Exit::Success && ids.len() == listed.len()).then_some(ids))
    }

    async fn hash_one(&self, path: &RepoPath) -> Result<Option<ObjectId>, GitError> {
        let finished = self
            .runner
            .finish(["hash-object", "--", path.as_str()])
            .await?;
        Ok(parse_id(finished.exit(), finished.into_stdout().ok()))
    }

    /// The commit a checked-out submodule is at; `None` when it is not a repository.
    async fn submodule_head(&self, path: &RepoPath) -> Result<Option<ObjectId>, GitError> {
        let dir = self.root().join(path.as_str());
        if !is_dir(&dir).await {
            return Ok(None);
        }
        let finished = GitRunner::new(dir)
            .finish(["rev-parse", "--verify", "--quiet", "HEAD"])
            .await?;
        Ok(parse_id(finished.exit(), finished.into_stdout().ok()))
    }
}

async fn is_dir(path: &Path) -> bool {
    fs::metadata(path).await.is_ok_and(|meta| meta.is_dir())
}

fn parse_id(
    exit: Exit,
    stdout: Option<crate::domains::repository::process::git_runner::Stdout>,
) -> Option<ObjectId> {
    match (exit, stdout) {
        (Exit::Success, Some(stdout)) => ObjectId::parse(&stdout.trimmed()).ok(),
        (Exit::Success | Exit::Failure(_) | Exit::Terminated, _) => None,
    }
}

fn set(hashes: &mut [Option<ObjectId>], i: usize, id: Option<ObjectId>) {
    if let Some(slot) = hashes.get_mut(i) {
        *slot = id;
    }
}

#[cfg(test)]
#[path = "worktree_hashes_tests.rs"]
mod tests;
