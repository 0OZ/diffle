//! `<git-dir>/diffle/comments.json`: every comparison's threads and viewed marks, one file.
//!
//! - [`storage_key::StorageKey`]: which comparison's set a store owns.
//! - [`versions_store`]: the file's shapes across versions 1 to 3, and how an old one upgrades.
//! - [`store_error::CommentError`]: what a store operation can fail with.
//! - [`comment_store::CommentStore`]: reads and locked read-modify-write of one key's set.

pub mod comment_store;
#[cfg(test)]
pub mod scratch_store;
pub mod storage_key;
pub mod store_error;
pub mod versions_store;
