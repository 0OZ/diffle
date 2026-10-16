//! A file's bytes on one side of a comparison: a Git object, or the checkout on disk.

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::cat_file_batch::ObjectKind;
use crate::domains::repository::process::git_error::GitError;

impl GitRepo {
    /// The file at `path` on `source`, or `None` when it is absent there or not a regular
    /// file (a directory, a gitlink). A symlink reads as its target text, which is what Git
    /// stores for it, so a link pointing outside the repository never exposes its target.
    pub async fn read_file(
        &self,
        source: &TreeSource,
        path: &RepoPath,
    ) -> Result<Option<Vec<u8>>, GitError> {
        match source {
            TreeSource::Worktree => self.read_worktree(path).await,
            TreeSource::Object(id) => self.blob_at(id, path, usize::MAX).await,
        }
    }

    /// The first `bytes` of the file at `path` on `source`, with the same absences as
    /// [`read_file`](Self::read_file); a symlink has no head.
    pub async fn file_head(
        &self,
        source: &TreeSource,
        path: &RepoPath,
        bytes: usize,
    ) -> Result<Option<Vec<u8>>, GitError> {
        match source {
            TreeSource::Worktree => self.worktree_head(path, bytes).await,
            TreeSource::Object(id) => self.blob_at(id, path, bytes).await,
        }
    }

    /// The first `bytes` of each blob, keyed by id. An id that is not a blob is left out.
    pub async fn blob_heads(
        &self,
        ids: &[ObjectId],
        bytes: usize,
    ) -> Result<HashMap<ObjectId, Vec<u8>>, GitError> {
        let mut heads = HashMap::new();
        for id in ids {
            if let Some(object) = self.objects.read(id.as_str(), bytes).await?
                && object.kind() == ObjectKind::Blob
            {
                heads.insert(id.clone(), object.into_head());
            }
        }
        Ok(heads)
    }

    async fn blob_at(
        &self,
        tree: &ObjectId,
        path: &RepoPath,
        keep: usize,
    ) -> Result<Option<Vec<u8>>, GitError> {
        let object = self.objects.read(&format!("{tree}:{path}"), keep).await?;
        Ok(object
            .filter(|object| object.kind() == ObjectKind::Blob)
            .map(|object| object.into_head()))
    }

    fn worktree_file(&self, path: &RepoPath) -> PathBuf {
        self.root().join(path.as_str())
    }

    async fn read_worktree(&self, path: &RepoPath) -> Result<Option<Vec<u8>>, GitError> {
        let file = self.worktree_file(path);
        let read = async {
            let metadata = fs::symlink_metadata(&file).await?;
            match (metadata.is_symlink(), metadata.is_file()) {
                (true, _) => Ok(Some(
                    fs::read_link(&file)
                        .await?
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                )),
                (false, true) => fs::read(&file).await.map(Some),
                (false, false) => Ok(None),
            }
        };
        absent_as_none(read.await, file)
    }

    async fn worktree_head(
        &self,
        path: &RepoPath,
        bytes: usize,
    ) -> Result<Option<Vec<u8>>, GitError> {
        let file = self.worktree_file(path);
        let read = async {
            if !fs::symlink_metadata(&file).await?.is_file() {
                return Ok(None);
            }
            let mut head = Vec::new();
            let limit = u64::try_from(bytes).unwrap_or(u64::MAX);
            fs::File::open(&file)
                .await?
                .take(limit)
                .read_to_end(&mut head)
                .await?;
            Ok(Some(head))
        };
        absent_as_none(read.await, file)
    }
}

/// A file that vanished, or whose parent is not a directory, is absent rather than an error.
fn absent_as_none<T>(read: io::Result<Option<T>>, file: PathBuf) -> Result<Option<T>, GitError> {
    match read {
        Ok(found) => Ok(found),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(source) => Err(GitError::Read { path: file, source }),
    }
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
