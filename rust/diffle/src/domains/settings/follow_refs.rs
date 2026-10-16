//! How a review between refs reacts when one of them moves.

use std::fmt;

/// `On` recomputes at once, as a worktree review follows edits. `Off` announces the move and
/// waits for a reload. `Auto` waits when an endpoint names a branch, whose pushes and
/// rebases should not swap the review out under the reviewer, and recomputes otherwise (a
/// detached `HEAD`, a tag, an expression).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FollowRefs {
    On,
    #[default]
    Auto,
    Off,
}

impl FollowRefs {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Auto => "auto",
            Self::Off => "off",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::On, Self::Auto, Self::Off]
            .into_iter()
            .find(|choice| choice.as_str() == text)
    }

    /// Whether a move of the refs recomputes the review, given whether an endpoint names a branch.
    #[must_use]
    pub const fn recomputes(self, names_a_branch: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Auto => !names_a_branch,
        }
    }
}

impl fmt::Display for FollowRefs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_everything_but_branches() {
        assert!(FollowRefs::On.recomputes(true));
        assert!(!FollowRefs::Off.recomputes(false));
        assert!(FollowRefs::Auto.recomputes(false));
        assert!(!FollowRefs::Auto.recomputes(true));
    }

    #[test]
    fn spells_and_parses_as_the_wire_does() {
        for choice in [FollowRefs::On, FollowRefs::Auto, FollowRefs::Off] {
            assert_eq!(FollowRefs::parse(choice.as_str()), Some(choice));
        }
        assert_eq!(FollowRefs::parse("sometimes"), None);
        assert_eq!(FollowRefs::default().to_string(), "auto");
    }
}
