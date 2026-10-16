//! The pages the server serves, and the browser that shows them.
//!
//! - [`client_dir`]: where the built web client lives on disk.
//! - [`browser_lifetime`]: when the last page going away ends a review.

pub mod browser_lifetime;
pub mod client_dir;
