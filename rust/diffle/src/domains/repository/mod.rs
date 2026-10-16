//! The Git repository on disk, and the vocabulary every other domain uses to name
//! things inside it. Git is only ever started from [`process`].

pub mod contents;
pub mod diffing;
pub mod git_refs;
pub mod git_repo;
pub mod history;
pub mod names;
pub mod process;
pub mod search;
#[cfg(test)]
pub mod test_repo;
