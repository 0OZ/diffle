//! Keeping threads on their text as the review changes.
//!
//! Everything here is pure: the caller reads files and diffs, hands the answers over as data
//! ([`reviewed_files::ReviewedFiles`]), and gets placements back.
//!
//! - [`quoting`]: cutting a quote out of a file, and finding a quote again.
//! - [`shown_ranges`]: the lines of a side a unified patch shows.
//! - [`reviewed_files`]: what relocation needs asked, and the answers.

pub mod quoting;
pub mod reviewed_files;
pub mod shown_ranges;
