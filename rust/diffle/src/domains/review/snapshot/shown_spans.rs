//! Which new-side lines each changed file's patch shows, kept for one generation of the snapshot.
//!
//! A search needs these for every changed file, but not the patches' text. Keeping only the
//! spans leaves a few bytes per hunk behind instead of the whole diff, and a second search
//! still finds them ready.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::OnceCell;

use super::patch_cache::{PatchCache, PatchSetting};
use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::changes::patches::patch_for;
use crate::domains::comments::relocation::shown_ranges::shown_ranges;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::search::line_span::LineSpan;
use crate::kernel::side::Side;

type Slot = Arc<OnceCell<Arc<[LineSpan]>>>;

#[derive(Debug, Default)]
pub struct ShownSpans {
    per_file: Mutex<HashMap<RepoPath, Slot>>,
}

impl ShownSpans {
    /// The spans one file's patch shows. A patch that was already read for the viewer is used as
    /// it is; otherwise it is read, reduced and dropped. A failure is not remembered.
    pub async fn of(
        &self,
        patches: &PatchCache,
        repo: &GitRepo,
        setting: &PatchSetting<'_>,
        file: &ChangedFile,
    ) -> Result<Arc<[LineSpan]>, GitError> {
        let slot = self.slot(&file.path);
        let spans = slot
            .get_or_try_init(|| async {
                let patch = match patches.cached(&file.path) {
                    Some(patch) => patch,
                    None => Arc::new(
                        patch_for(repo, setting.sides, file, setting.context.get(), None).await?,
                    ),
                };
                Ok::<_, GitError>(Arc::from(spans_of(&patch)))
            })
            .await?;
        Ok(Arc::clone(spans))
    }

    fn slot(&self, path: &RepoPath) -> Slot {
        let mut slots = self.per_file.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(slots.entry(path.clone()).or_default())
    }
}

/// The new-side line spans a unified patch shows, hunk by hunk, context included.
#[must_use]
pub fn spans_of(patch: &str) -> Vec<LineSpan> {
    shown_ranges(patch, Side::New)
        .into_iter()
        .filter_map(|range| LineSpan::new(range.start().get(), range.end().get()).ok())
        .collect()
}
