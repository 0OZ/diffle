//! What a thread points at.

use crate::kernel::side::Side;

use super::lines::{Line, LineRange};
use super::review_path::ReviewPath;

/// The exact text of a line range as it was when the thread was opened.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Quote(String);

impl Quote {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A line range on one side of a file, with the text it quoted so it can be found again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineAnchor {
    path: ReviewPath,
    side: Side,
    range: LineRange,
    quoted: Quote,
}

impl LineAnchor {
    #[must_use]
    pub fn new(path: ReviewPath, side: Side, range: LineRange, quoted: Quote) -> Self {
        Self {
            path,
            side,
            range,
            quoted,
        }
    }

    #[must_use]
    pub fn path(&self) -> &ReviewPath {
        &self.path
    }

    #[must_use]
    pub fn side(&self) -> Side {
        self.side
    }

    #[must_use]
    pub fn range(&self) -> LineRange {
        self.range
    }

    #[must_use]
    pub fn quoted(&self) -> &Quote {
        &self.quoted
    }

    /// The same anchor on other lines: the quoted text moved, it did not change.
    #[must_use]
    pub fn moved_to(&self, range: LineRange) -> Self {
        Self {
            range,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    Line(LineAnchor),
    /// The file as a whole, with no line: GitHub's file-level comment.
    File(ReviewPath),
}

impl Anchor {
    #[must_use]
    pub fn path(&self) -> &ReviewPath {
        match self {
            Self::Line(anchor) => anchor.path(),
            Self::File(path) => path,
        }
    }

    /// The first line of a line anchor; a file anchor has none.
    #[must_use]
    pub fn start_line(&self) -> Option<Line> {
        match self {
            Self::Line(anchor) => Some(anchor.range().start()),
            Self::File(_) => None,
        }
    }

    /// The line a thread sorts under: a file thread comes before every line of its file.
    #[must_use]
    pub fn sort_line(&self) -> u64 {
        self.start_line().map_or(0, Line::get)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(number: u64) -> Line {
        Line::new(number).unwrap()
    }

    fn line_anchor() -> LineAnchor {
        LineAnchor::new(
            ReviewPath::parse("a.py").unwrap(),
            Side::Old,
            LineRange::single(line(2)),
            Quote::new("b"),
        )
    }

    #[test]
    fn a_moved_anchor_keeps_everything_but_its_lines() {
        let anchor = line_anchor();
        let moved = anchor.moved_to(LineRange::single(line(7)));
        assert_eq!(moved.range(), LineRange::single(line(7)));
        assert_eq!(moved.path(), anchor.path());
        assert_eq!(moved.side(), Side::Old);
        assert_eq!(moved.quoted().as_str(), "b");
    }

    #[test]
    fn an_anchor_knows_its_path_and_where_it_sorts() {
        let on_lines = Anchor::Line(line_anchor());
        let on_file = Anchor::File(ReviewPath::parse("a.py").unwrap());
        assert_eq!(on_lines.path(), on_file.path());
        assert_eq!(on_lines.start_line(), Some(line(2)));
        assert_eq!(on_file.start_line(), None);
        assert_eq!((on_lines.sort_line(), on_file.sort_line()), (2, 0));
        assert_eq!(
            (Side::Old.to_string(), Side::New.to_string()),
            ("old".into(), "new".into())
        );
    }
}
