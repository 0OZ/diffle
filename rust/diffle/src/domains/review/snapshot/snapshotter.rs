//! Keeps the current [`Snapshot`] of one mode, and the patches derived from it, until
//! something invalidates them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use tokio::sync::OnceCell;

use crate::domains::changes::generated_files::generated_detector::GeneratedDetector;
use crate::domains::repository::diffing::diff_sides::DiffSides;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::search::line_span::LineSpan;
use crate::domains::review::mode::review_mode::ReviewMode;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::snapshot::patch_cache::{PatchCache, PatchSetting};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::review::snapshot::shown_spans::ShownSpans;
use crate::domains::review::snapshot::snapshot_compute::compute_snapshot;
use crate::domains::review::snapshot::snapshot_version::SnapshotVersion;
use crate::domains::settings::context_lines::ContextLines;

/// Patches computed ahead of any request, for the files a reviewer opens first.
const PREWARM: usize = 3;

/// One version of the snapshot and everything cached for it.
#[derive(Debug)]
struct Generation {
    version: SnapshotVersion,
    context: ContextLines,
    snapshot: OnceCell<Arc<Snapshot>>,
    patches: PatchCache,
    shown: ShownSpans,
    prewarmed: AtomicBool,
}

impl Generation {
    fn new(version: SnapshotVersion, context: ContextLines) -> Arc<Self> {
        Arc::new(Self {
            version,
            context,
            snapshot: OnceCell::new(),
            patches: PatchCache::default(),
            shown: ShownSpans::default(),
            prewarmed: AtomicBool::new(false),
        })
    }
}

/// Turns a mode and a repository into snapshots. Cached until [`invalidate`](Self::invalidate);
/// a request that started under an older generation still finishes with that generation's data.
#[derive(Debug)]
pub struct Snapshotter {
    repo: Arc<GitRepo>,
    mode: ReviewMode,
    /// A blob never changes, so a refresh re-reads only content it has not seen.
    detector: GeneratedDetector,
    generation: RwLock<Arc<Generation>>,
}

impl Snapshotter {
    #[must_use]
    pub fn new(
        repo: Arc<GitRepo>,
        mode: ReviewMode,
        version: SnapshotVersion,
        context: ContextLines,
    ) -> Self {
        Self {
            repo,
            mode,
            detector: GeneratedDetector::default(),
            generation: RwLock::new(Generation::new(version, context)),
        }
    }

    #[must_use]
    pub const fn mode(&self) -> &ReviewMode {
        &self.mode
    }

    #[must_use]
    pub fn context(&self) -> ContextLines {
        self.latest().context
    }

    /// Changes the context lines: drops cached patches, and the next snapshot carries the value.
    pub fn set_context(&self, context: ContextLines, version: SnapshotVersion) {
        if context != self.context() {
            self.replace(Generation::new(version, context));
        }
    }

    /// Forgets the snapshot and its patches; the next request computes `version`.
    pub fn invalidate(&self, version: SnapshotVersion) {
        self.replace(Generation::new(version, self.context()));
    }

    pub async fn current(&self) -> Result<Arc<Snapshot>, ReviewError> {
        Ok(self.settled().await?.1)
    }

    /// The patch of one changed file, or `None` for a path that is not one.
    pub async fn patch(&self, path: &RepoPath) -> Result<Option<Arc<String>>, ReviewError> {
        let (generation, snapshot) = self.settled().await?;
        let Some(file) = snapshot.changed_file(path) else {
            return Ok(None);
        };
        let sides = sides_of(&snapshot);
        let setting = setting(&sides, &generation);
        Ok(Some(
            generation.patches.file(&self.repo, &setting, file).await?,
        ))
    }

    /// The new-side line spans one changed file's patch shows, or `None` for a path that is not
    /// one. Unlike [`patch`](Self::patch) it keeps no patch text: a search asks for every file's.
    pub async fn shown_spans(
        &self,
        path: &RepoPath,
    ) -> Result<Option<Arc<[LineSpan]>>, ReviewError> {
        let (generation, snapshot) = self.settled().await?;
        let Some(file) = snapshot.changed_file(path) else {
            return Ok(None);
        };
        let sides = sides_of(&snapshot);
        let setting = setting(&sides, &generation);
        let spans = generation
            .shown
            .of(&generation.patches, &self.repo, &setting, file)
            .await?;
        Ok(Some(spans))
    }

    /// Every changed file's patch, concatenated.
    pub async fn patch_all(&self) -> Result<Arc<String>, ReviewError> {
        let (generation, snapshot) = self.settled().await?;
        let sides = sides_of(&snapshot);
        let setting = setting(&sides, &generation);
        Ok(generation
            .patches
            .everything(&self.repo, &setting, &snapshot.parts().changed)
            .await?)
    }

    /// The patches of `paths`, concatenated. Unknown paths are skipped, repeated ones once.
    pub async fn patch_many(&self, paths: &[RepoPath]) -> Result<String, ReviewError> {
        let (generation, snapshot) = self.settled().await?;
        let mut seen = std::collections::HashSet::new();
        let files: Vec<_> = paths
            .iter()
            .filter(|path| seen.insert(*path))
            .filter_map(|path| snapshot.changed_file(path))
            .collect();
        let sides = sides_of(&snapshot);
        let setting = setting(&sides, &generation);
        Ok(generation
            .patches
            .many(&self.repo, &setting, &files)
            .await?)
    }

    fn latest(&self) -> Arc<Generation> {
        Arc::clone(
            &self
                .generation
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    fn replace(&self, next: Arc<Generation>) {
        *self
            .generation
            .write()
            .unwrap_or_else(PoisonError::into_inner) = next;
    }

    /// The latest generation's snapshot, computing it once however many ask. A failure is not
    /// remembered.
    async fn settled(&self) -> Result<(Arc<Generation>, Arc<Snapshot>), ReviewError> {
        let generation = self.latest();
        let snapshot = generation
            .snapshot
            .get_or_try_init(|| async {
                compute_snapshot(
                    &self.repo,
                    &self.detector,
                    &self.mode,
                    generation.version,
                    generation.context,
                )
                .await
                .map(Arc::new)
            })
            .await?;
        let snapshot = Arc::clone(snapshot);
        self.prewarm(&generation, &snapshot);
        Ok((generation, snapshot))
    }

    /// Starts the first files' patches without waiting for them; a failure surfaces when one is asked for.
    fn prewarm(&self, generation: &Arc<Generation>, snapshot: &Arc<Snapshot>) {
        if generation.prewarmed.swap(true, Ordering::AcqRel) {
            return;
        }
        for file in snapshot.parts().changed.iter().take(PREWARM) {
            let (repo, generation, snapshot, path) = (
                Arc::clone(&self.repo),
                Arc::clone(generation),
                Arc::clone(snapshot),
                file.path.clone(),
            );
            tokio::spawn(async move {
                let Some(file) = snapshot.changed_file(&path) else {
                    return;
                };
                let sides = sides_of(&snapshot);
                let setting = setting(&sides, &generation);
                if let Err(error) = generation.patches.file(&repo, &setting, file).await {
                    tracing::debug!(%path, %error, "patch prewarm failed");
                }
            });
        }
    }
}

fn sides_of(snapshot: &Snapshot) -> DiffSides {
    let parts = snapshot.parts();
    DiffSides::new(&parts.old, &parts.new)
}

fn setting<'a>(sides: &'a DiffSides, generation: &Generation) -> PatchSetting<'a> {
    PatchSetting {
        sides,
        context: generation.context,
    }
}

#[cfg(test)]
#[path = "snapshotter_tests.rs"]
mod tests;
