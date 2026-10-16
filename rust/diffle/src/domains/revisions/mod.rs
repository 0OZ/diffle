//! What is being compared. A reviewer names revisions; this domain parses them into a
//! [`revspec::Revspec`], resolves them against the repository, and fixes the
//! [`comment_key::CommentKey`] the review's comments live under.
//!
//! The vocabulary lives here; turning a request into a [`comparison::Comparison`] is
//! [`resolution`].

pub mod comment_key;
pub mod comparison;
pub mod entry_request;
pub mod resolution;
pub mod revision;
pub mod revspec;
pub mod revspec_error;
