//! Threads that arrive from outside — the browser's composer, an agent's JSON — and are only
//! kept once the snapshot confirms where they point.
//!
//! - [`new_thread`]: a validated request to open a thread.
//! - [`duplicates`]: when a request repeats an open thread.
//! - [`anchor_answers`]: what a batch must ask of the snapshot, and the answers.
//! - [`import_wire`]: the JSON payload, parsed with the same messages as the TypeScript server.

pub mod anchor_answers;
pub mod duplicates;
pub mod import_wire;
pub mod new_thread;
