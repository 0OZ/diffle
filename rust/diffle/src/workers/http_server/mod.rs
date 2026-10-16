//! The HTTP server: binds a port, guards the API and the socket by `Host` and `Origin`, serves
//! the client build, and pushes session events to every connected page.
//!
//! - [`listening`]: the port and the address.
//! - [`pages`]: the client build and the lifetime of the tab that opened it.
//! - [`guard_layer`], [`ws_hub`]: the request guard and the live socket.
//! - [`server`]: puts them together and runs; [`server_error`] says why it could not.

pub mod guard_layer;
pub mod listening;
pub mod pages;
pub mod server;
pub mod server_error;
pub mod ws_hub;

#[cfg(test)]
mod server_fixtures;
#[cfg(test)]
mod server_tests;
