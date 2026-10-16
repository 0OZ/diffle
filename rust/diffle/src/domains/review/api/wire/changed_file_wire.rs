//! `ChangedFileSchema`.

use serde::Serialize;

use crate::domains::changes::changed_file::{ChangedFile, Delta};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFileWire<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_path: Option<&'a str>,
    status: String,
    additions: u32,
    deletions: u32,
    binary: bool,
    /// The new side's blob; empty for a deletion, or a worktree file Git could not hash.
    blob: &'a str,
    old_blob: &'a str,
    generated: bool,
    /// Present, and `true`, for a gitlink only.
    #[serde(skip_serializing_if = "Option::is_none")]
    submodule: Option<bool>,
}

impl<'a> From<&'a ChangedFile> for ChangedFileWire<'a> {
    fn from(file: &'a ChangedFile) -> Self {
        let (additions, deletions) = match file.delta {
            Delta::Text {
                additions,
                deletions,
            } => (additions, deletions),
            Delta::Binary => (0, 0),
        };
        Self {
            path: file.path.as_str(),
            old_path: file.kind.from_path().map(|from| from.as_str()),
            status: file.kind.code().to_string(),
            additions,
            deletions,
            binary: file.is_binary(),
            blob: file.blob.as_ref().map_or("", |blob| blob.as_str()),
            old_blob: file.old_blob.as_ref().map_or("", |blob| blob.as_str()),
            generated: file.generated,
            submodule: file.submodule.then_some(true),
        }
    }
}
