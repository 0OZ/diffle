//! A conversation anchored to a line range or a whole file.
//!
//! - [`ids`]: opaque thread and message ids.
//! - [`body`]: the text a comment is written with ([`body::Body`], never blank) and the text a
//!   message holds ([`body::MessageText`], which an edit may blank).
//! - [`message`]: one message, and the non-empty list a thread keeps.
//! - [`thread`]: the thread, its resolution and freshness, and review order.
//! - [`thread_query`]: which threads a read asks for.
//! - [`thread_wire`], [`request_wire`]: the JSON shapes of threads and of requests about them.

pub mod body;
pub mod ids;
pub mod message;
pub mod request_wire;
pub mod thread;
pub mod thread_query;
pub mod thread_wire;
