//! Flagging the changed files a generator wrote, so the review can collapse them.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use super::looks_generated::{SNIFF_BYTES, looks_generated, path_looks_generated};
use crate::domains::changes::changed_file::ChangedFile;
use crate::domains::repository::contents::listings::AttrValue;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::kernel::bounded::map_limit;

/// The gitattribute GitHub reads to collapse generated files; `=false` opts a file out of
/// every heuristic.
const GENERATED_ATTRIBUTE: &str = "linguist-generated";
/// Concurrent worktree reads while sniffing.
const SNIFF_CONCURRENCY: usize = 8;
/// Verdicts remembered by blob; the map is emptied, not evicted, past this.
const VERDICTS_MAX: usize = 20_000;

/// Decides `generated` for changed files. A blob never changes, so a refresh only reads
/// content it has not seen.
#[derive(Debug, Default)]
pub struct GeneratedDetector {
    verdicts: Mutex<HashMap<ObjectId, bool>>,
}

impl GeneratedDetector {
    /// Sets `generated` on every textual file. A `linguist-generated` attribute on the new
    /// side decides outright, as on GitHub; otherwise a path convention, then a content sniff.
    /// It never fails the snapshot: a failed lookup or read falls to the next heuristic.
    pub async fn flag(&self, repo: &GitRepo, files: &mut [ChangedFile], new: &TreeSource) {
        let paths: Vec<_> = files
            .iter()
            .filter(|file| file.is_textual())
            .map(|file| &file.path)
            .collect();
        let marked = repo
            .bool_attribute(GENERATED_ATTRIBUTE, &paths, new)
            .await
            .unwrap_or_else(|error| {
                tracing::debug!("cannot read {GENERATED_ATTRIBUTE}: {error}");
                HashMap::new()
            });
        let mut sniff = Vec::new();
        for file in files.iter_mut().filter(|file| file.is_textual()) {
            match marked.get(file.path.as_str()) {
                Some(AttrValue::Set) => file.generated = true,
                Some(AttrValue::Unset) => file.generated = false,
                None => {
                    file.generated = path_looks_generated(file.path.as_str());
                    if !file.generated {
                        sniff.push(file);
                    }
                }
            }
        }
        let unknown = self.apply_known(sniff);
        let heads = read_heads(repo, &unknown, new).await;
        for (file, head) in unknown.into_iter().zip(heads) {
            self.classify(file, head.as_deref());
        }
    }

    /// Applies remembered verdicts and returns the files still to be read.
    fn apply_known<'a>(&self, sniff: Vec<&'a mut ChangedFile>) -> Vec<&'a mut ChangedFile> {
        let verdicts = self.verdicts.lock().unwrap_or_else(PoisonError::into_inner);
        let mut unknown = Vec::new();
        for file in sniff {
            match file.blob.as_ref().and_then(|blob| verdicts.get(blob)) {
                Some(&verdict) => file.generated = verdict,
                None => unknown.push(file),
            }
        }
        unknown
    }

    fn classify(&self, file: &mut ChangedFile, head: Option<&[u8]>) {
        file.generated = looks_generated(file.path.as_str(), head);
        let (Some(_), Some(blob)) = (head, &file.blob) else {
            return;
        };
        let mut verdicts = self.verdicts.lock().unwrap_or_else(PoisonError::into_inner);
        if verdicts.len() >= VERDICTS_MAX {
            verdicts.clear();
        }
        verdicts.insert(blob.clone(), file.generated);
    }
}

/// The first bytes of each file, in order; `None` where they could not be read.
async fn read_heads(
    repo: &GitRepo,
    files: &[&mut ChangedFile],
    new: &TreeSource,
) -> Vec<Option<Vec<u8>>> {
    match new {
        TreeSource::Worktree => {
            let paths = files.iter().map(|file| file.path.clone()).collect();
            map_limit(paths, SNIFF_CONCURRENCY, |path| async move {
                repo.file_head(&TreeSource::Worktree, &path, SNIFF_BYTES)
                    .await
                    .ok()
                    .flatten()
            })
            .await
        }
        TreeSource::Object(_) => {
            let blobs: Vec<ObjectId> = files.iter().filter_map(|file| file.blob.clone()).collect();
            // A missing blob reads as not generated rather than failing the snapshot.
            let mut heads = repo
                .blob_heads(&blobs, SNIFF_BYTES)
                .await
                .unwrap_or_default();
            files
                .iter()
                .map(|file| file.blob.as_ref().and_then(|blob| heads.remove(blob)))
                .collect()
        }
    }
}

#[cfg(test)]
#[path = "generated_detector_tests.rs"]
mod tests;
