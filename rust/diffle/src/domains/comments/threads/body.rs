//! The text of comments.

use thiserror::Error;

/// What a new comment or reply says: never blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Body(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BodyError {
    #[error("body required")]
    Blank,
}

impl Body {
    pub fn parse(text: impl Into<String>) -> Result<Self, BodyError> {
        let text = text.into();
        if trimmed(&text).is_empty() {
            return Err(BodyError::Blank);
        }
        Ok(Self(text))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a stored message says. An edit may leave it blank, as the TypeScript server allows, so
/// reading such a file must not fail; only writing a new comment demands a [`Body`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageText(String);

impl MessageText {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<Body> for MessageText {
    fn from(body: Body) -> Self {
        Self(body.0)
    }
}

/// `text` without the whitespace JavaScript's `String.prototype.trim` drops, so a body is
/// judged blank, and compared, exactly as the TypeScript server does.
#[must_use]
pub fn trimmed(text: &str) -> &str {
    // JavaScript drops U+FEFF and keeps U+0085: the reverse of Rust's `trim`.
    text.trim_matches(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_not_blank() {
        assert_eq!(Body::parse("  \n\t"), Err(BodyError::Blank));
        assert_eq!(Body::parse(""), Err(BodyError::Blank));
        assert_eq!(Body::parse(" x ").unwrap().as_str(), " x ");
        assert_eq!(BodyError::Blank.to_string(), "body required");
    }

    #[test]
    fn a_message_may_hold_any_text() {
        assert_eq!(MessageText::new("").as_str(), "");
        let body = Body::parse("hello").unwrap();
        assert_eq!(MessageText::from(body).as_str(), "hello");
    }

    #[test]
    fn trimming_follows_javascript() {
        assert_eq!(trimmed("\u{feff} a \u{a0}"), "a");
        assert_eq!(trimmed("\u{85}a"), "\u{85}a");
        assert_eq!(trimmed("\u{2028}a\r\n"), "a");
    }
}
