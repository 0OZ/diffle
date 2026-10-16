//! A Git object name: the lowercase hex digest of a commit, tree, or blob.

use std::fmt;

use thiserror::Error;

const SHA1_HEX_LEN: usize = 40;
const SHA256_HEX_LEN: usize = 64;
/// A revision typed at least this long that starts a hash names that object by its hash,
/// so the reference cannot move.
pub const MIN_ABBREVIATION_LEN: usize = 7;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectId(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObjectIdError {
    #[error("an object id has {SHA1_HEX_LEN} or {SHA256_HEX_LEN} characters, not {0}")]
    Length(usize),
    #[error("an object id is hexadecimal")]
    NotHex,
}

impl ObjectId {
    pub fn parse(text: &str) -> Result<Self, ObjectIdError> {
        match text.len() {
            SHA1_HEX_LEN | SHA256_HEX_LEN => {}
            other => return Err(ObjectIdError::Length(other)),
        }
        if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ObjectIdError::NotHex);
        }
        Ok(Self(text.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether `typed` is long enough, and a prefix of this id, to name it by its hash.
    pub fn is_abbreviated_by(&self, typed: &str) -> bool {
        typed.len() >= MIN_ABBREVIATION_LEN && self.0.starts_with(&typed.to_ascii_lowercase())
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    #[test]
    fn parses_both_hash_widths_and_lowercases() {
        assert_eq!(
            ObjectId::parse(&SHA1.to_uppercase()).unwrap().as_str(),
            SHA1
        );
        assert!(ObjectId::parse(&"a".repeat(SHA256_HEX_LEN)).is_ok());
    }

    #[test]
    fn rejects_other_lengths_and_non_hex() {
        assert_eq!(ObjectId::parse("abc"), Err(ObjectIdError::Length(3)));
        assert_eq!(
            ObjectId::parse(&"g".repeat(SHA1_HEX_LEN)),
            Err(ObjectIdError::NotHex)
        );
    }

    #[test]
    fn an_abbreviation_needs_the_minimum_length_and_a_matching_prefix() {
        let id = ObjectId::parse(SHA1).unwrap();
        assert!(id.is_abbreviated_by("4b825dc"));
        assert!(id.is_abbreviated_by("4B825DC642"));
        assert!(!id.is_abbreviated_by("4b825d"));
        assert!(!id.is_abbreviated_by("4b825de"));
        assert!(!id.is_abbreviated_by("main"));
    }
}
