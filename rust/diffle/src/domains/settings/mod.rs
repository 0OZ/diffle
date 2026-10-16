//! The reviewer's machine-wide preferences: how much context to show, which files start
//! viewed, how a review between refs reacts to a move, and which language servers to run.

pub mod config_store;
pub mod context_lines;
pub mod follow_refs;
pub mod language_id;
pub mod user_config;
pub mod user_config_wire;
