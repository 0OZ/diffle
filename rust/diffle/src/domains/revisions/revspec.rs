//! `git diff` style revision arguments, parsed into the comparison they name.

use std::fmt;

use super::revision::Revision;
use super::revspec_error::RevspecError;
use crate::domains::repository::names::rev_name::RevName;

const THREE_DOTS: &str = "...";
const TWO_DOTS: &str = "..";
const COMMIT_SUFFIX: &str = "^!";

/// What the reviewer asked to compare. Each shape says how the old side is found, so a
/// comparison of the worktree against itself, or a "commit" that is the worktree, cannot be
/// written down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Revspec {
    /// `old` against `new` exactly as named (`a..b`, `a b`).
    Direct { old: Revision, new: Revision },
    /// What `new` added since it left `old` (`a...b`, a lone `a`): the old side is
    /// merge-base(`old`, `new`), with the worktree standing for `HEAD`.
    MergeBase { old: Revision, new: Revision },
    /// One commit against its first parent, as `git show` shows it (`a^!`).
    Commit { rev: RevName },
}

impl Revspec {
    /// The uncommitted changes: `HEAD..worktree`.
    pub fn working() -> Self {
        Self::Direct {
            old: Revision::head(),
            new: Revision::Worktree,
        }
    }

    /// The commit `rev` names, against its first parent.
    pub fn commit(rev: RevName) -> Self {
        Self::Commit { rev }
    }

    /// Parses `diffle`'s revision arguments:
    ///
    /// | arguments | meaning |
    /// |---|---|
    /// | `a` | merge-base(`a`, `HEAD`) against `HEAD`, i.e. `a...HEAD` |
    /// | `a..b` | `a` against `b` |
    /// | `a...b` | merge-base(`a`, `b`) against `b` |
    /// | `a b` | `a` against `b` |
    /// | `a^!` | `a` against its first parent |
    /// | `worktree` | `HEAD` against the uncommitted tree |
    ///
    /// An empty side of `..` or `...` is `HEAD`.
    pub fn parse(args: &[String]) -> Result<Self, RevspecError> {
        match args {
            [] | [_, _, _, ..] => Err(RevspecError::ArgCount),
            [old, new] => Self::between(old, new),
            [single] => Self::single(single),
        }
    }

    fn between(old: &str, new: &str) -> Result<Self, RevspecError> {
        let named = [old, new];
        if named.iter().any(|arg| arg.contains(TWO_DOTS)) {
            return Err(RevspecError::RangeWithTwoRevisions);
        }
        if named.iter().any(|arg| arg.ends_with(COMMIT_SUFFIX)) {
            return Err(RevspecError::CommitWithTwoRevisions);
        }
        Ok(Self::Direct {
            old: Revision::parse(old)?,
            new: Revision::parse(new)?,
        })
    }

    fn single(arg: &str) -> Result<Self, RevspecError> {
        if let Some(commit) = arg.strip_suffix(COMMIT_SUFFIX) {
            return Self::one_commit(commit);
        }
        if let Some((old, new)) = arg.split_once(THREE_DOTS) {
            return Ok(Self::MergeBase {
                old: Revision::parse_or_head(old)?,
                new: Revision::parse_or_head(new)?,
            });
        }
        if let Some((old, new)) = arg.split_once(TWO_DOTS) {
            return Ok(Self::Direct {
                old: Revision::parse_or_head(old)?,
                new: Revision::parse_or_head(new)?,
            });
        }
        // A lone revision reviews what it and HEAD diverged into: the everyday "my branch" diff.
        Ok(match Revision::parse(arg)? {
            Revision::Worktree => Self::working(),
            old @ Revision::Rev(_) => Self::MergeBase {
                old,
                new: Revision::head(),
            },
        })
    }

    fn one_commit(text: &str) -> Result<Self, RevspecError> {
        if text.contains(TWO_DOTS) {
            return Err(RevspecError::CommitWithRange);
        }
        match Revision::parse_or_head(text)? {
            Revision::Worktree => Err(RevspecError::WorktreeIsNotACommit),
            Revision::Rev(rev) => Ok(Self::Commit { rev }),
        }
    }

    /// The revisions the reviewer named, in order; the worktree is not a revision.
    pub fn named_revisions(&self) -> Vec<&RevName> {
        match self {
            Self::Direct { old, new } | Self::MergeBase { old, new } => [old, new]
                .into_iter()
                .filter_map(|side| match side {
                    Revision::Rev(name) => Some(name),
                    Revision::Worktree => None,
                })
                .collect(),
            Self::Commit { rev } => vec![rev],
        }
    }

    /// Whether the new side is the uncommitted tree, so the range has uncommitted changes to focus.
    pub fn ends_at_worktree(&self) -> bool {
        match self {
            Self::Direct { new, .. } | Self::MergeBase { new, .. } => new.is_worktree(),
            Self::Commit { .. } => false,
        }
    }

    /// Whether either side is the uncommitted tree, which keeps the review live.
    pub fn touches_worktree(&self) -> bool {
        match self {
            Self::Direct { old, new } | Self::MergeBase { old, new } => {
                old.is_worktree() || new.is_worktree()
            }
            Self::Commit { .. } => false,
        }
    }
}

impl fmt::Display for Revspec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct { old, new } => write!(f, "{old}{TWO_DOTS}{new}"),
            Self::MergeBase { old, new } => write!(f, "{old}{THREE_DOTS}{new}"),
            Self::Commit { rev } => write!(f, "{rev}{COMMIT_SUFFIX}"),
        }
    }
}

#[cfg(test)]
#[path = "revspec_tests.rs"]
mod tests;
