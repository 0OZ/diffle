//! The name a comparison's comments are filed under in `comments.json`.

use std::fmt;

/// An opaque key such as `working` or `range:...`. The caller derives it from the comparison;
/// this domain only files by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StorageKey(String);

impl StorageKey {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_kept_as_given() {
        let key = StorageKey::new("pr:abc");
        assert_eq!(key.as_str(), "pr:abc");
        assert_eq!(key.to_string(), "pr:abc");
    }
}
