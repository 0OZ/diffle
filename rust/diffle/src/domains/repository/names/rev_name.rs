//! A revision expression handed to Git: `main`, `HEAD~2`, `origin/feat`, an abbreviated hash.

use std::fmt;

use thiserror::Error;

use super::object_id::ObjectId;

const HEAD: &str = "HEAD";
const PARENT_SUFFIX: &str = "^";
const ANCESTOR_MARK: char = '~';

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RevNameError {
    #[error("a revision cannot be empty")]
    Empty,
    #[error("a revision cannot start with \"-\"")]
    OptionShaped,
    #[error("a revision cannot contain a control character")]
    ControlCharacter,
}

impl RevName {
    /// Spaces are fine (`HEAD@{2 days ago}`); anything Git could read as an option or a
    /// second argument is not.
    pub fn parse(text: &str) -> Result<Self, RevNameError> {
        if text.is_empty() {
            return Err(RevNameError::Empty);
        }
        if text.starts_with('-') {
            return Err(RevNameError::OptionShaped);
        }
        if text.chars().any(char::is_control) {
            return Err(RevNameError::ControlCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn head() -> Self {
        Self(HEAD.to_owned())
    }

    pub fn of_object(id: &ObjectId) -> Self {
        Self(id.as_str().to_owned())
    }

    pub fn is_head(&self) -> bool {
        self.0 == HEAD
    }

    /// The first parent of whatever this names.
    pub fn parent(&self) -> Self {
        Self(format!("{}{PARENT_SUFFIX}", self.0))
    }

    /// The commit `generations` first-parent steps behind whatever this names.
    pub fn ancestor(&self, generations: u32) -> Self {
        Self(format!("{}{ANCESTOR_MARK}{generations}", self.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_parent_and_the_nth_ancestor() {
        assert_eq!(RevName::head().parent().as_str(), "HEAD^");
        assert_eq!(RevName::head().ancestor(3).as_str(), "HEAD~3");
    }

    #[test]
    fn accepts_expressions_with_spaces() {
        assert_eq!(
            RevName::parse("HEAD@{2 days ago}").unwrap().as_str(),
            "HEAD@{2 days ago}"
        );
    }

    #[test]
    fn rejects_what_git_would_read_as_something_else() {
        assert_eq!(RevName::parse(""), Err(RevNameError::Empty));
        assert_eq!(
            RevName::parse("--output=x"),
            Err(RevNameError::OptionShaped)
        );
        assert_eq!(RevName::parse("a\nb"), Err(RevNameError::ControlCharacter));
    }

    #[test]
    fn names_a_parent_and_recognises_head() {
        assert_eq!(RevName::parse("main").unwrap().parent().as_str(), "main^");
        assert!(RevName::head().is_head());
        assert!(!RevName::parse("main").unwrap().is_head());
    }
}
