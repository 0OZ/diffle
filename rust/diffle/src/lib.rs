//! Diffle's review server, read like a book: this page is the table of contents.
//!
//! - [`app`]: the composition root. It names every step of a run and holds no logic.
//! - [`kernel`]: what every part leans on (the error type, logging). It leans on nothing.
//! - [`workers`]: long-running tasks the program starts (the file-system watcher, the HTTP server).
//! - [`domains`]: the bounded contexts, each a self-contained slice with its own types,
//!   rules, and use cases.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod app;
pub mod domains;
pub mod kernel;
pub mod workers;
