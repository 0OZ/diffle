//! The name of one side's bytes, which the picture URL carries.

use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::kernel::side::Side;

impl Snapshot {
    /// Names the bytes of one side of `path`, or `None` when that side has no name: a changed
    /// file's blob on that side, else the commit of a side that is one, since an unchanged
    /// path is alike on both. What a key names never changes, so a picture served under it
    /// may be cached for good.
    #[must_use]
    pub fn image_key(&self, path: &RepoPath, side: Side) -> Option<String> {
        if let Some(file) = self.changed_file(path) {
            let blob = match side {
                Side::New => file.blob.as_ref(),
                Side::Old => file.old_blob.as_ref(),
            };
            return blob.map(|blob| blob.as_str().to_owned());
        }
        let parts = self.parts();
        [&parts.new, &parts.old]
            .into_iter()
            .find_map(|source| source.object())
            .map(|commit| commit.as_str().to_owned())
    }
}
