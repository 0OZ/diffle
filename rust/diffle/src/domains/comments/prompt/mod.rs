//! A review's threads as the text an agent receives.
//!
//! - [`agent_prompt::format_prompt`]: one block per thread.
//! - [`suggestions`]: splitting a body into prose and ```suggestion fences.

pub mod agent_prompt;
pub mod suggestions;
