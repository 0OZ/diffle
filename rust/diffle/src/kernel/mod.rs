//! What every part of the server leans on. Nothing in here knows a domain's rules.

pub mod allowed_origin;
pub mod atomic_file;
pub mod bounded;
pub mod epoch_millis;
pub mod file_lock;
pub mod logging;
pub mod request_guard;
pub mod side;
