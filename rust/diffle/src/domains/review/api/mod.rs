//! The review over HTTP: the routes the browser client calls, and the JSON they speak.
//! Handlers stay thin; every rule they apply lives in the domains they call.

pub mod api_state;
pub mod contents;
pub mod error_api;
pub mod input_api;
pub mod response_api;
pub mod router_api;
pub mod settings;
pub mod snapshots;
pub mod threads;
pub mod unsupported_api;
pub mod wire;

/// Shared by the route tests here and by the server tests that mount these routes.
#[cfg(test)]
pub mod api_fixtures;
#[cfg(test)]
mod router_tests;
