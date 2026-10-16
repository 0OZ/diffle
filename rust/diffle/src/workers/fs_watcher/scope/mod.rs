//! What a watcher observes and which paths count as a change.
//!
//! - [`watch_target`]: the two things that can be watched, and the ignore rule of a checkout.
//! - [`metadata_paths`]: the Git metadata that moves a review.
//! - [`event_filter`]: the verdict on one path.
//! - [`checkout_walk`]: the directories of a checkout worth a watch.

pub mod checkout_walk;
pub mod event_filter;
pub mod metadata_paths;
pub mod watch_target;
