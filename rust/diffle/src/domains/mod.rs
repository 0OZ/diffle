//! The bounded contexts of a review, one folder each.
//!
//! - [`repository`]: the Git repository on disk and the vocabulary for talking to it.
//!   The hub: every other domain reads it, it reads nobody (see `.rustlayout`).
//! - [`revisions`]: what is being compared — revision arguments, their resolution to
//!   commits, and the key a review's comments are stored under.
//! - [`comments`]: threads on the lines and files under review, kept across refreshes in
//!   `comments.json`, and the prompt they export as.
//! - [`changes`]: which files differ between two sides, and their patches.
//! - [`review`]: the review in progress, its snapshots, and the serialized transitions between
//!   them. The view: it reads every sibling, nobody reads it.
//! - [`settings`]: the reviewer's machine-wide preferences.

pub mod changes;
pub mod comments;
pub mod repository;
pub mod review;
pub mod revisions;
pub mod settings;
