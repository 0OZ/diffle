//! The run's language servers: lifecycle, routing, and coalesced status.
mod lifecycle;
mod queries;
mod state;

pub use state::{LspPool, PoolOptions};
