//! Reading what a tree holds: a file on either side of a comparison, the first bytes of
//! blobs, the hashes Git would give the worktree's files, and what kind of bytes they are.

pub mod binary;
pub mod files;
pub mod image_type;
pub mod listings;
pub mod worktree_hashes;
