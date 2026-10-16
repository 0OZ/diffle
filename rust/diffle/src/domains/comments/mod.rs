//! Review comments: threads anchored to lines or files, their relocation as the review
//! changes, viewed marks, and the file they live in.
//!
//! - [`anchors`]: where a comment points ([`anchors::anchor::Anchor`], one-based lines).
//! - [`threads`]: the conversation on an anchor, and the JSON of threads and requests.
//! - [`relocation`]: finding a quote again, and the data it works from.
//! - [`imports`]: new threads from the browser or an agent, validated before they are kept.
//! - [`viewed`]: the viewed marks of a review.
//! - [`prompt`]: threads as the text an agent receives.
//! - [`comment_set`]: one comparison's threads and marks, and the rules for changing them.
//! - [`storage`]: [`storage::comment_store::CommentStore`], the locked `comments.json`.
//! - [`comment_error`]: why a change is refused ([`comment_error::NotFound`], [`comment_error::Unquotable`]).
//!
//! The domain reads no repository and calls nobody: what relocation and imports need from the
//! review arrives as plain data, and the caller keys the store with a [`storage::storage_key::StorageKey`].

pub mod anchors;
pub mod comment_error;
pub mod comment_set;
pub mod imports;
pub mod prompt;
pub mod relocation;
#[cfg(test)]
pub mod sample_threads;
pub mod storage;
pub mod threads;
pub mod viewed;
