//! Whether a file looks machine-written.
//!
//! - [`looks_generated`]: the rules, by path and by a generator's marker near the top.
//! - [`generated_detector`]: applies them to a snapshot's files, honouring `linguist-generated`.

pub mod generated_detector;
pub mod looks_generated;
