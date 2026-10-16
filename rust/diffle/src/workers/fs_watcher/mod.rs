//! Turns file-system events into one debounced "dirty" signal.
//!
//! - [`scope`]: what is watched and which paths count (targets, ignored sets, git metadata).
//! - [`watcher`]: the handle a session holds: start, receive signals, close.
//! - [`event_loop`]: the single task that owns the OS watcher and the debounce timer.
//! - [`observation`], [`debounce`], [`tree_watch`]: its parts, one responsibility each.
//! - [`error`]: why a watcher could not start.

mod debounce;
pub mod error;
mod event_loop;
mod observation;
pub mod scope;
mod tree_watch;
pub mod watcher;

#[cfg(test)]
mod test_support;
