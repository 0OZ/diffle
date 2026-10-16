//! Where a comment points: a file in the review, or a range of lines on one side of it.
//!
//! - [`review_path::ReviewPath`]: the path a thread is filed under.
//! - [`lines`]: one-based [`lines::Line`]s and the [`lines::LineRange`]s that cannot be inverted.
//! - [`anchor::Anchor`]: a line range with the text it quoted, or a whole file.
//! - [`anchor_wire`]: the JSON shape of an anchor.

pub mod anchor;
pub mod anchor_wire;
pub mod lines;
pub mod review_path;
