//! What differs between the two sides of a comparison: the changed files, the new side's
//! tree, which files are generated, and the patches.
//!
//! - [`changed_files`] lists them, reading Git through [`git_listing`].
//! - [`generated_files`] marks the machine-written ones.
//! - [`patches`] renders their diffs.

pub mod change_kind;
pub mod changed_file;
pub mod changed_files;
pub mod generated_files;
pub mod git_listing;
pub mod patches;
