//! Reading a comparison's files out of Git's output.
//!
//! - [`raw_parse`]: `diff --raw` and `--numstat` records into changed files.
//! - [`untracked_size`]: the line counts Git does not report for untracked files.
//! - [`tree_paths`]: the new side's paths, with the changed files merged in.

pub mod raw_parse;
pub mod tree_paths;
pub mod untracked_size;
