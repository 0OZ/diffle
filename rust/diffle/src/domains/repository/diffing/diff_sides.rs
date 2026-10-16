//! The two sides of a `git diff`, and how each pairing is spelled to Git.

use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::tree_source::TreeSource;

/// What a diff compares. The checkout can be either side but not both: the pairing that
/// would compare it with itself is [`Nothing`](Self::Nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSides {
    Nothing,
    Between {
        old: ObjectId,
        new: ObjectId,
    },
    ObjectToWorktree(ObjectId),
    /// Git only diffs a tree against the checkout, so this runs reversed (`-R`).
    WorktreeToObject(ObjectId),
}

impl DiffSides {
    #[must_use]
    pub fn new(old: &TreeSource, new: &TreeSource) -> Self {
        match (old, new) {
            (TreeSource::Worktree, TreeSource::Worktree) => Self::Nothing,
            (TreeSource::Object(old), TreeSource::Object(new)) => Self::Between {
                old: old.clone(),
                new: new.clone(),
            },
            (TreeSource::Object(old), TreeSource::Worktree) => Self::ObjectToWorktree(old.clone()),
            (TreeSource::Worktree, TreeSource::Object(new)) => Self::WorktreeToObject(new.clone()),
        }
    }

    #[must_use]
    pub const fn is_reversed(&self) -> bool {
        matches!(self, Self::WorktreeToObject(_))
    }

    #[must_use]
    pub const fn touches_worktree(&self) -> bool {
        matches!(self, Self::ObjectToWorktree(_) | Self::WorktreeToObject(_))
    }

    /// The revision arguments of `git diff`.
    pub(super) fn range_args(&self) -> Vec<String> {
        match self {
            Self::Nothing => Vec::new(),
            Self::Between { old, new } => vec![old.to_string(), new.to_string()],
            Self::ObjectToWorktree(old) => vec![old.to_string()],
            Self::WorktreeToObject(new) => vec!["-R".to_owned(), new.to_string()],
        }
    }

    /// Fixed header prefixes: `-R` swaps Git's too, and the rendered sides stay named `a/` and
    /// `b/`. `--no-ext-diff` keeps a configured external tool from replacing the unified diff.
    pub(super) fn patch_args(&self) -> [&'static str; 3] {
        match self.is_reversed() {
            true => ["--no-ext-diff", "--src-prefix=b/", "--dst-prefix=a/"],
            false => ["--no-ext-diff", "--src-prefix=a/", "--dst-prefix=b/"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: &str) -> ObjectId {
        ObjectId::parse(&byte.repeat(40)).unwrap()
    }

    #[test]
    fn each_pairing_has_its_own_spelling() {
        let (a, b) = (id("a"), id("b"));
        let object = |id: &ObjectId| TreeSource::Object(id.clone());
        let between = DiffSides::new(&object(&a), &object(&b));
        assert_eq!(between.range_args(), [a.to_string(), b.to_string()]);
        let forward = DiffSides::new(&object(&a), &TreeSource::Worktree);
        assert_eq!(forward.range_args(), [a.to_string()]);
        let backward = DiffSides::new(&TreeSource::Worktree, &object(&b));
        assert_eq!(backward.range_args(), ["-R".to_owned(), b.to_string()]);
        assert!(backward.is_reversed() && backward.touches_worktree());
        assert!(!between.touches_worktree() && !between.is_reversed());
    }

    #[test]
    fn the_worktree_against_itself_compares_nothing() {
        let sides = DiffSides::new(&TreeSource::Worktree, &TreeSource::Worktree);
        assert_eq!(sides, DiffSides::Nothing);
        assert!(sides.range_args().is_empty());
        assert!(!sides.touches_worktree());
    }

    #[test]
    fn a_reversed_diff_keeps_the_old_side_named_a() {
        let reversed = DiffSides::WorktreeToObject(id("c"));
        assert_eq!(
            reversed.patch_args()[1..],
            ["--src-prefix=b/", "--dst-prefix=a/"]
        );
    }
}
