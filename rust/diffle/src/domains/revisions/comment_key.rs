//! The key a review's comments are stored under. It names the comparison by branch identity
//! where it can, so comments survive pushes, rebases, and local branch aliases.
//!
//! Keys read like the comparison, with `...` for a merge base and `..` otherwise:
//!
//! | comparison | key |
//! |---|---|
//! | `origin/main...HEAD`, tracking `o/r`'s `feat` | `range:o/r:main...o/r:feat` |
//! | `main...feat`, no GitHub upstream | `range:refs/heads/main...refs/heads/feat` |
//! | `main..worktree` | `range:refs/heads/main..worktree` |
//! | `<C>..<E>`, hashes | `range:<C>..<E>` |
//! | one commit | `commit:<hash>` |
//! | the uncommitted changes | `working` |

use std::fmt;

use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::tree_source::WORKTREE;

const WORKING: &str = "working";
const COMMIT_PREFIX: &str = "commit:";
const RANGE_PREFIX: &str = "range:";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommentKey(String);

/// How the old side of a keyed range was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeShape {
    Direct,
    MergeBase,
}

/// One end of a keyed range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySide {
    /// A branch identity: `owner/repo:branch` for a GitHub upstream, else the full ref.
    Branch(String),
    Object(ObjectId),
    Worktree,
}

impl CommentKey {
    pub fn working() -> Self {
        Self(WORKING.to_owned())
    }

    pub fn commit(id: &ObjectId) -> Self {
        Self(format!("{COMMIT_PREFIX}{id}"))
    }

    pub fn range(old: &KeySide, new: &KeySide, shape: RangeShape) -> Self {
        let dots = match shape {
            RangeShape::Direct => "..",
            RangeShape::MergeBase => "...",
        };
        Self(format!("{RANGE_PREFIX}{old}{dots}{new}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for KeySide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Branch(identity) => f.write_str(identity),
            Self::Object(id) => id.fmt(f),
            Self::Worktree => f.write_str(WORKTREE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    #[test]
    fn keys_read_like_the_comparison() {
        let main = KeySide::Branch("refs/heads/main".into());
        let id = ObjectId::parse(SHA).unwrap();
        assert_eq!(CommentKey::working().as_str(), "working");
        assert_eq!(CommentKey::commit(&id).as_str(), format!("commit:{SHA}"));
        assert_eq!(
            CommentKey::range(&main, &KeySide::Worktree, RangeShape::Direct).as_str(),
            "range:refs/heads/main..worktree"
        );
        assert_eq!(
            CommentKey::range(&KeySide::Object(id), &main, RangeShape::MergeBase).as_str(),
            format!("range:{SHA}...refs/heads/main")
        );
    }
}
