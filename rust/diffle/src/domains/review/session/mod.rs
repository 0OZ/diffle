//! The running review: serialized transitions, a watcher that keeps it current, and the
//! allowlisted reads over its snapshot.

pub mod ignored_paths;
pub mod review_session;
pub mod session_events;
pub mod session_live;
pub mod session_reading;
pub mod session_transitions;
pub mod session_watch;

#[cfg(test)]
mod session_fixtures;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod session_watch_tests;
