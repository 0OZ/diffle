//! The path a thread is filed under: a file of the review, as its new path names it.

use std::cmp::Ordering;
use std::fmt;

use thiserror::Error;

/// A non-empty path. It orders the way JavaScript orders strings (by UTF-16 code unit), so
/// threads sort the same under the TypeScript server that reads the same file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReviewPath(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a path cannot be empty")]
pub struct EmptyPath;

impl ReviewPath {
    pub fn parse(path: impl Into<String>) -> Result<Self, EmptyPath> {
        let path = path.into();
        if path.is_empty() {
            return Err(EmptyPath);
        }
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Ord for ReviewPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.encode_utf16().cmp(other.0.encode_utf16())
    }
}

impl PartialOrd for ReviewPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for ReviewPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_not_empty() {
        assert_eq!(ReviewPath::parse(""), Err(EmptyPath));
        assert_eq!(ReviewPath::parse("a.py").unwrap().as_str(), "a.py");
        assert_eq!(ReviewPath::parse("a.py").unwrap().to_string(), "a.py");
    }

    #[test]
    fn paths_order_by_utf16_unit_like_javascript() {
        let path = |text: &str| ReviewPath::parse(text).unwrap();
        assert!(path("a.py") < path("b.py"));
        // U+FF5E sorts after an astral char by code point, before it by UTF-16 unit.
        assert!(path("\u{1F600}") < path("\u{FF5E}"));
        assert!("\u{1F600}" > "\u{FF5E}");
    }
}
