//! Serving one review, from the first listen to the last word on stdout.
//!
//! - [`serve_review`]: the run, step by step.
//! - [`startup_report`]: the lines that say what is being reviewed.
//! - [`ending`]: what ends a review.
//! - [`hand_off`]: the open comments, as the prompt the review exists to produce.

pub mod ending;
pub mod hand_off;
pub mod serve_review;
pub mod startup_report;
