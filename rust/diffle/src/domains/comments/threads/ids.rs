//! Opaque ids. A new one is a UUID; one read from disk or a URL is kept as it is.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(String);

impl ThreadId {
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl MessageId {
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ThreadId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<String> for MessageId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_unique_and_stored_ids_are_kept() {
        assert_ne!(ThreadId::generate(), ThreadId::generate());
        assert_ne!(MessageId::generate(), MessageId::generate());
        assert_eq!(ThreadId::from("c1".to_owned()).as_str(), "c1");
        assert_eq!(MessageId::from("m1".to_owned()).to_string(), "m1");
        assert_eq!(ThreadId::from("t".to_owned()).to_string(), "t");
        assert_eq!(MessageId::generate().as_str().len(), 36);
    }
}
