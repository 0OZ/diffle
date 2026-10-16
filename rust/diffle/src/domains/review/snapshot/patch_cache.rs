//! The patches of one snapshot generation, computed once and shared by every request for them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::OnceCell;

use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::changes::patches::{patch_all, patch_for, patch_many};
use crate::domains::repository::diffing::diff_sides::DiffSides;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::settings::context_lines::ContextLines;

type Slot = Arc<OnceCell<Arc<String>>>;

/// What a generation's patches are made of: which sides, and how much context.
#[derive(Debug, Clone)]
pub struct PatchSetting<'a> {
    pub sides: &'a DiffSides,
    pub context: ContextLines,
}

#[derive(Debug, Default)]
pub struct PatchCache {
    per_file: Mutex<HashMap<RepoPath, Slot>>,
    all: OnceCell<Arc<String>>,
}

impl PatchCache {
    /// One file's patch. A failure is not remembered, so the next request tries again.
    pub async fn file(
        &self,
        repo: &GitRepo,
        setting: &PatchSetting<'_>,
        file: &ChangedFile,
    ) -> Result<Arc<String>, GitError> {
        let slot = self.slot(&file.path);
        let patch = slot
            .get_or_try_init(|| async {
                patch_for(repo, setting.sides, file, setting.context.get(), None)
                    .await
                    .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(patch))
    }

    /// Every changed file's patch, concatenated.
    pub async fn everything(
        &self,
        repo: &GitRepo,
        setting: &PatchSetting<'_>,
        files: &[ChangedFile],
    ) -> Result<Arc<String>, GitError> {
        let patch = self
            .all
            .get_or_try_init(|| async {
                patch_all(repo, setting.sides, files, setting.context.get())
                    .await
                    .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(patch))
    }

    /// The patches of `files`, concatenated: cached ones as they are, the rest in one Git
    /// call. A lone file goes through the per-file cache, so a later single request finds it.
    pub async fn many(
        &self,
        repo: &GitRepo,
        setting: &PatchSetting<'_>,
        files: &[&ChangedFile],
    ) -> Result<String, GitError> {
        let mut cached = Vec::new();
        let mut rest = Vec::new();
        for file in files {
            match self.cached(&file.path) {
                Some(patch) => cached.push(patch),
                None => rest.push(*file),
            }
        }
        let fresh = match rest.as_slice() {
            [] => String::new(),
            [only] => self.file(repo, setting, only).await?.as_str().to_owned(),
            several => patch_many(repo, setting.sides, several, setting.context.get()).await?,
        };
        let mut joined: String = cached.iter().map(|patch| patch.as_str()).collect();
        joined.push_str(&fresh);
        Ok(joined)
    }

    fn slot(&self, path: &RepoPath) -> Slot {
        let mut slots = self.per_file.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(slots.entry(path.clone()).or_default())
    }

    pub(super) fn cached(&self, path: &RepoPath) -> Option<Arc<String>> {
        let slots = self.per_file.lock().unwrap_or_else(PoisonError::into_inner);
        slots.get(path).and_then(|slot| slot.get().cloned())
    }
}
