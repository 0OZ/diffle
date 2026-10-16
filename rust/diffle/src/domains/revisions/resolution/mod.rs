//! Turning what the reviewer asked for into a [`Comparison`](super::comparison::Comparison):
//! resolve the sides to commits, then key the comments.

pub mod comment_keying;
#[cfg(test)]
mod resolution_tests;
pub mod resolve_endpoints;
pub mod resolve_review;
pub mod resolved_endpoints;
