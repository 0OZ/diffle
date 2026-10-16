//! One comparison's comments in `<git-dir>/diffle/comments.json`.

use std::io;
use std::path::{Path, PathBuf};

use tokio::fs;

use super::storage_key::StorageKey;
use super::store_error::CommentError;
use super::versions_store::{SetStore, StoredFile};
use crate::domains::comments::anchors::anchor::Anchor;
use crate::domains::comments::comment_set::{CommentSet, ImportOutcome};
use crate::domains::comments::imports::anchor_answers::AnchorAnswers;
use crate::domains::comments::imports::new_thread::NewThread;
use crate::domains::comments::relocation::reviewed_files::{RelocationNeeds, ReviewedFiles};
use crate::domains::comments::threads::body::{Body, MessageText};
use crate::domains::comments::threads::ids::{MessageId, ThreadId};
use crate::domains::comments::threads::thread::CommentThread;
use crate::domains::comments::threads::thread_query::ThreadQuery;
use crate::domains::comments::viewed::viewed_marks::ViewedEntry;
use crate::kernel::atomic_file::write_file_atomic;
use crate::kernel::epoch_millis::EpochMillis;
use crate::kernel::file_lock::with_file_lock;

const STATE_DIR: &str = "diffle";
const FILE_NAME: &str = "comments.json";

/// What an edit to the set concluded: its result, and whether the file must be rewritten.
struct Edit<R> {
    result: R,
    changed: bool,
}

impl<R> Edit<R> {
    fn saved(result: R) -> Self {
        Self {
            result,
            changed: true,
        }
    }

    fn when(result: R, changed: bool) -> Self {
        Self { result, changed }
    }
}

/// Owns this key's set in `comments.json`: threads and viewed marks under `sets[key]`. Nothing
/// is cached: every read parses the file, and every mutation is a read-modify-write of fresh
/// disk state under `comments.json.lock`, touching only its own key. So any number of stores on
/// one file (two servers on one repository, an old-mode store still finishing a write while its
/// replacement starts) see and keep each other's writes.
#[derive(Debug, Clone)]
pub struct CommentStore {
    file: PathBuf,
    key: StorageKey,
}

impl CommentStore {
    /// Opens the store; an older file is backed up to `comments.v<n>.bak` and rewritten in the
    /// current format first.
    pub async fn open(git_dir: &Path, key: &StorageKey) -> Result<Self, CommentError> {
        let file = git_dir.join(STATE_DIR).join(FILE_NAME);
        with_file_lock(&file, || upgrade_in_place(&file)).await??;
        Ok(Self {
            file,
            key: key.clone(),
        })
    }

    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Threads matching `query`, ordered by path, line, then creation.
    pub async fn threads(&self, query: &ThreadQuery) -> Result<Vec<CommentThread>, CommentError> {
        Ok(self.read().await?.matching(query))
    }

    pub async fn get(&self, id: &ThreadId) -> Result<Option<CommentThread>, CommentError> {
        Ok(self.read().await?.find(id).cloned())
    }

    pub async fn viewed(&self) -> Result<Vec<ViewedEntry>, CommentError> {
        Ok(self.read().await?.viewed().entries().to_vec())
    }

    pub async fn add_thread(
        &self,
        anchor: Anchor,
        body: Body,
    ) -> Result<CommentThread, CommentError> {
        self.mutate(|set| {
            Ok(Edit::saved(set.add_thread(
                anchor,
                body,
                EpochMillis::now(),
            )))
        })
        .await
    }

    pub async fn reply(&self, id: &ThreadId, body: Body) -> Result<CommentThread, CommentError> {
        self.mutate(|set| Ok(Edit::saved(set.reply(id, body, EpochMillis::now())?)))
            .await
    }

    pub async fn edit_message(
        &self,
        id: &ThreadId,
        message: &MessageId,
        text: MessageText,
    ) -> Result<CommentThread, CommentError> {
        self.mutate(|set| {
            Ok(Edit::saved(set.edit_message(
                id,
                message,
                text,
                EpochMillis::now(),
            )?))
        })
        .await
    }

    /// Removes the message; removes the thread when its last message goes (returns `None` then).
    pub async fn remove_message(
        &self,
        id: &ThreadId,
        message: &MessageId,
    ) -> Result<Option<CommentThread>, CommentError> {
        self.mutate(|set| Ok(Edit::saved(set.remove_message(id, message)?)))
            .await
    }

    pub async fn set_resolved(
        &self,
        id: &ThreadId,
        resolved: bool,
    ) -> Result<CommentThread, CommentError> {
        self.mutate(|set| {
            Ok(Edit::saved(set.set_resolved(
                id,
                resolved,
                EpochMillis::now(),
            )?))
        })
        .await
    }

    pub async fn remove_thread(&self, id: &ThreadId) -> Result<(), CommentError> {
        self.mutate(|set| Ok(Edit::saved(set.remove_thread(id)?)))
            .await
    }

    /// Opens a thread for each request that is not already an open duplicate, placing every
    /// anchor through `answers` (see [`AnchorQuestions`]). When any cannot be placed, nothing is
    /// added and [`CommentError::Unquotable`] is returned.
    ///
    /// [`AnchorQuestions`]: crate::domains::comments::imports::anchor_answers::AnchorQuestions
    pub async fn import_threads(
        &self,
        requests: Vec<NewThread>,
        answers: &AnchorAnswers,
    ) -> Result<ImportOutcome, CommentError> {
        self.mutate(|set| {
            let outcome = set.import(requests, answers, EpochMillis::now())?;
            let changed = !outcome.added.is_empty();
            Ok(Edit::when(outcome, changed))
        })
        .await
    }

    pub async fn clear(&self) -> Result<(), CommentError> {
        self.mutate(|set| {
            set.clear();
            Ok(Edit::saved(()))
        })
        .await
    }

    /// Drops every stale thread. Returns how many went.
    pub async fn remove_stale(&self) -> Result<usize, CommentError> {
        self.mutate(|set| {
            let removed = set.remove_stale();
            Ok(Edit::when(removed, removed > 0))
        })
        .await
    }

    /// Marks or unmarks a path as viewed at a given new-side blob. The newest previous entry for
    /// the path is kept so a later blob can be recognised as re-touched. Returns every mark.
    pub async fn set_viewed(&self, entry: ViewedEntry) -> Result<Vec<ViewedEntry>, CommentError> {
        self.mutate(|set| Ok(Edit::saved(set.mark_viewed(entry))))
            .await
    }

    /// Replaces the marks of many paths at once; their history is dropped. Returns every mark.
    pub async fn set_viewed_many(
        &self,
        entries: Vec<ViewedEntry>,
    ) -> Result<Vec<ViewedEntry>, CommentError> {
        self.mutate(|set| Ok(Edit::saved(set.mark_viewed_many(entries))))
            .await
    }

    /// The files and sides the stored threads point at: read them, then call
    /// [`CommentStore::relocate_all`].
    pub async fn relocation_needs(&self) -> Result<RelocationNeeds, CommentError> {
        Ok(self.read().await?.relocation_needs())
    }

    /// Re-anchors every thread `reviewed` has answers for, against the set as it is now under
    /// the lock: a thread added or moved by another store meanwhile is re-anchored too, never
    /// clobbered. Returns true if anything changed.
    pub async fn relocate_all(&self, reviewed: &ReviewedFiles) -> Result<bool, CommentError> {
        self.mutate(|set| {
            let changed = set.relocate(reviewed);
            Ok(Edit::when(changed, changed))
        })
        .await
    }

    /// This key's set as on disk right now; empty when the file is absent. Malformed JSON is an
    /// error.
    async fn read(&self) -> Result<CommentSet, CommentError> {
        let stored = read_stored(&self.file).await?;
        self.set_of(&stored)
    }

    fn set_of(&self, stored: &StoredFile) -> Result<CommentSet, CommentError> {
        let set = stored
            .set(self.key.as_str())
            .map_err(|source| malformed(&self.file, source))?;
        CommentSet::try_from(set).map_err(|source| CommentError::Invalid {
            path: self.file.clone(),
            source,
        })
    }

    /// Locked read-modify-write: `edit` changes this key's set as read from disk under the lock,
    /// other keys pass through untouched, and the file is rewritten when the edit says it
    /// changed something. A failing edit writes nothing. A failed write fails for its caller
    /// only; the next mutation still reaches disk.
    async fn mutate<R>(
        &self,
        edit: impl FnOnce(&mut CommentSet) -> Result<Edit<R>, CommentError>,
    ) -> Result<R, CommentError> {
        with_file_lock(&self.file, || async {
            let stored = read_stored(&self.file)
                .await?
                .upgrade()
                .map_err(|source| malformed(&self.file, source))?;
            let mut set = self.set_of(&stored)?;
            let edit = edit(&mut set)?;
            if edit.changed {
                let updated = stored
                    .with_set(self.key.as_str(), &SetStore::from(&set))
                    .map_err(|source| malformed(&self.file, source))?;
                write_stored(&self.file, &updated).await?;
            }
            Ok(edit.result)
        })
        .await?
    }
}

/// Rewrites an older file as the current format, keeping the original as `comments.v<n>.bak`.
async fn upgrade_in_place(file: &Path) -> Result<(), CommentError> {
    let stored = read_stored(file).await?;
    if stored.is_current() {
        return Ok(());
    }
    let backup = file.with_file_name(format!("comments.v{}.bak", stored.version()));
    fs::copy(file, &backup)
        .await
        .map_err(|source| io_error(file, source))?;
    let upgraded = stored.upgrade().map_err(|source| malformed(file, source))?;
    write_stored(file, &upgraded).await
}

/// The file as stored, or an empty store when absent.
async fn read_stored(file: &Path) -> Result<StoredFile, CommentError> {
    match fs::read_to_string(file).await {
        Ok(text) => StoredFile::parse(&text).map_err(|source| malformed(file, source)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(StoredFile::empty()),
        Err(source) => Err(io_error(file, source)),
    }
}

/// Atomic write, owner-only: the file quotes source lines.
async fn write_stored(file: &Path, stored: &StoredFile) -> Result<(), CommentError> {
    let text = stored.render().map_err(|source| malformed(file, source))?;
    write_file_atomic(file, &text)
        .await
        .map_err(|source| io_error(file, source))
}

fn malformed(file: &Path, source: serde_json::Error) -> CommentError {
    CommentError::Malformed {
        path: file.to_owned(),
        source,
    }
}

fn io_error(file: &Path, source: io::Error) -> CommentError {
    CommentError::Io {
        path: file.to_owned(),
        source,
    }
}

#[cfg(test)]
#[path = "store_import_tests.rs"]
mod import_tests;
#[cfg(test)]
#[path = "store_migration_tests.rs"]
mod migration_tests;
#[cfg(test)]
#[path = "store_relocation_tests.rs"]
mod relocation_tests;
#[cfg(test)]
#[path = "store_sharing_tests.rs"]
mod sharing_tests;
#[cfg(test)]
#[path = "comment_store_tests.rs"]
mod tests;
