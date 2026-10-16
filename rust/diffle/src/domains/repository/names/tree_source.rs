//! A place a tree is read from.

use std::fmt;

use super::object_id::ObjectId;

/// How the uncommitted tree is spelled wherever a revision is expected.
pub const WORKTREE: &str = "worktree";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSource {
    /// The checkout on disk, staged and untracked files included; it has no hash.
    Worktree,
    /// A commit, or the empty tree standing in for the side before a root commit.
    Object(ObjectId),
}

impl TreeSource {
    #[must_use]
    pub const fn is_worktree(&self) -> bool {
        matches!(self, Self::Worktree)
    }

    #[must_use]
    pub const fn object(&self) -> Option<&ObjectId> {
        match self {
            Self::Worktree => None,
            Self::Object(id) => Some(id),
        }
    }
}

impl fmt::Display for TreeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Worktree => f.write_str(WORKTREE),
            Self::Object(id) => write!(f, "{id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_the_worktree_and_an_object_as_the_wire_does() {
        let id = ObjectId::parse(&"ab".repeat(20)).unwrap();
        assert_eq!(TreeSource::Worktree.to_string(), "worktree");
        assert_eq!(TreeSource::Object(id.clone()).to_string(), id.as_str());
        assert!(TreeSource::Worktree.is_worktree());
        assert_eq!(TreeSource::Object(id.clone()).object(), Some(&id));
        assert_eq!(TreeSource::Worktree.object(), None);
    }
}
