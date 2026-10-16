//! How a file changed. A rename or copy carries where it came from, so a plain edit cannot
//! name an old path.

use crate::domains::repository::names::repo_path::RepoPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    /// The file's type changed (a regular file became a symlink).
    TypeChanged,
    Unmerged,
    Renamed {
        from: RepoPath,
    },
    Copied {
        from: RepoPath,
    },
}

impl ChangeKind {
    /// The status letter Git prints and the wire carries.
    #[must_use]
    pub const fn code(&self) -> char {
        match self {
            Self::Added => 'A',
            Self::Modified => 'M',
            Self::Deleted => 'D',
            Self::TypeChanged => 'T',
            Self::Unmerged => 'U',
            Self::Renamed { .. } => 'R',
            Self::Copied { .. } => 'C',
        }
    }

    /// Reads a raw status (`M`, `R100`); a rename or copy needs its source path.
    #[must_use]
    pub fn parse(status: &str, from: Option<RepoPath>) -> Option<Self> {
        let kind = match (status.chars().next()?, from) {
            ('A', _) => Self::Added,
            ('M', _) => Self::Modified,
            ('D', _) => Self::Deleted,
            ('T', _) => Self::TypeChanged,
            ('U', _) => Self::Unmerged,
            ('R', Some(from)) => Self::Renamed { from },
            ('C', Some(from)) => Self::Copied { from },
            _ => return None,
        };
        Some(kind)
    }

    /// The source of a rename or copy.
    #[must_use]
    pub const fn from_path(&self) -> Option<&RepoPath> {
        match self {
            Self::Renamed { from } | Self::Copied { from } => Some(from),
            Self::Added | Self::Modified | Self::Deleted | Self::TypeChanged | Self::Unmerged => {
                None
            }
        }
    }

    #[must_use]
    pub const fn is_added(&self) -> bool {
        matches!(self, Self::Added)
    }

    #[must_use]
    pub const fn is_deleted(&self) -> bool {
        matches!(self, Self::Deleted)
    }

    /// Whether this is the kind that has no counterpart on the sibling side of a comparison
    /// whose `reversed` worktree is the old side: there an untracked file reads as deleted.
    #[must_use]
    pub const fn is_missing_on_one_side(&self, reversed: bool) -> bool {
        match reversed {
            true => self.is_deleted(),
            false => self.is_added(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(text: &str) -> RepoPath {
        RepoPath::parse(text).unwrap()
    }

    #[test]
    fn reads_every_status_letter_and_keeps_the_source_of_renames() {
        for (text, code) in [("A", 'A'), ("M", 'M'), ("D", 'D'), ("T", 'T'), ("U", 'U')] {
            assert_eq!(ChangeKind::parse(text, None).unwrap().code(), code);
        }
        let renamed = ChangeKind::parse("R087", Some(path("old.rs"))).unwrap();
        assert_eq!(renamed.code(), 'R');
        assert_eq!(renamed.from_path(), Some(&path("old.rs")));
        assert_eq!(
            ChangeKind::parse("C100", Some(path("src.rs")))
                .unwrap()
                .code(),
            'C'
        );
    }

    #[test]
    fn rejects_unknown_letters_and_a_rename_without_a_source() {
        assert_eq!(ChangeKind::parse("X", None), None);
        assert_eq!(ChangeKind::parse("", None), None);
        assert_eq!(ChangeKind::parse("R100", None), None);
    }

    #[test]
    fn a_reversed_comparison_misses_files_on_the_other_side() {
        assert!(ChangeKind::Added.is_missing_on_one_side(false));
        assert!(ChangeKind::Deleted.is_missing_on_one_side(true));
        assert!(!ChangeKind::Added.is_missing_on_one_side(true));
        assert!(!ChangeKind::Modified.is_missing_on_one_side(false));
    }
}
