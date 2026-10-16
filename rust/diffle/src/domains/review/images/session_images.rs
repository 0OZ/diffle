//! Reading a picture under its key.

use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::review::session::review_session::Session;
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::kernel::side::Side;

impl Session {
    /// The bytes `key` names for `path` on `side` of `snapshot`, or `None` when the key is not
    /// that side's current name, the path is not exposed, or the file is gone. Never reads
    /// outside the snapshot's allowlist.
    pub async fn read_image(
        &self,
        snapshot: &Snapshot,
        path: &RepoPath,
        side: Side,
        key: &str,
    ) -> Result<Option<Vec<u8>>, GitError> {
        if snapshot.image_key(path, side).as_deref() != Some(key) {
            return Ok(None);
        }
        // An unchanged path's key is a commit: read that commit, not a worktree that may have
        // moved on.
        if snapshot.changed_file(path).is_none() {
            let keyed = match snapshot.parts().new.object() {
                Some(commit) if commit.as_str() == key => Side::New,
                Some(_) | None => Side::Old,
            };
            return self.read_side(snapshot, path, keyed).await;
        }
        let Some(bytes) = self.read_side(snapshot, path, side).await? else {
            return Ok(None);
        };
        if !snapshot.source(side).is_worktree() {
            return Ok(Some(bytes));
        }
        // The file may have changed since `snapshot`: serve it only while it still hashes to
        // the key.
        let Some(target) = snapshot.side_path(path, side) else {
            return Ok(None);
        };
        let hashed = self.repo().hash_content(&bytes, &target).await?;
        Ok(hashed.filter(|id| id.as_str() == key).map(|_| bytes))
    }
}
