//! The JSON the client reads: one module per shape in `src/shared/protocol.ts`.

pub mod changed_file_wire;
pub mod commits_wire;
pub mod mode_wire;
pub mod server_message_wire;
pub mod side_wire;
pub mod snapshot_wire;

#[cfg(test)]
mod wire_tests;
