//! Where the server listens and how it is addressed.
//!
//! - [`bind`]: taking the default port with a fallback, or exactly the one asked for.
//! - [`server_url`]: the address the server tells people, and the one a browser on this machine uses.

pub mod bind;
pub mod server_url;
