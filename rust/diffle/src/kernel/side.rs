//! A column of a diff: the old text or the new.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Old,
    New,
}

impl Side {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Old => "old",
            Self::New => "new",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::Old, Self::New]
            .into_iter()
            .find(|side| side.as_str() == text)
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_and_parses_both_sides() {
        assert_eq!(Side::parse("old"), Some(Side::Old));
        assert_eq!(Side::parse("new"), Some(Side::New));
        assert_eq!(Side::parse("both"), None);
        assert_eq!(Side::New.to_string(), "new");
        assert!(Side::Old < Side::New);
    }
}
