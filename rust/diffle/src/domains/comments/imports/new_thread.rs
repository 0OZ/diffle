//! A request to open a thread, before the snapshot has confirmed where it points.

use thiserror::Error;

use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::anchors::lines::{InvertedRange, Line, LineRange};
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::threads::body::Body;
use crate::kernel::side::Side;

/// Where a new thread asks to be placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadTarget {
    /// The file as a whole.
    File,
    Lines {
        side: Side,
        range: LineRange,
        /// The text the browser showed; the server quotes the range itself when it is absent.
        quoted: Option<Quote>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TargetError {
    #[error(transparent)]
    Inverted(#[from] InvertedRange),
    #[error("side, endLine and quoted need a startLine")]
    NeedsStartLine,
}

impl ThreadTarget {
    /// Reads the optional fields of a payload. Without a start line the thread is on the file
    /// as a whole, and `side`, `end` and `quoted` are refused rather than ignored, so a payload
    /// that meant a line never lands on the file. `side` defaults to the new side and `end` to
    /// the start line.
    pub fn from_fields(
        start: Option<Line>,
        end: Option<Line>,
        side: Option<Side>,
        quoted: Option<Quote>,
    ) -> Result<Self, TargetError> {
        match (start, end, side, quoted) {
            (None, None, None, None) => Ok(Self::File),
            (None, ..) => Err(TargetError::NeedsStartLine),
            (Some(start), end, side, quoted) => Ok(Self::Lines {
                side: side.unwrap_or(Side::New),
                range: LineRange::new(start, end.unwrap_or(start))?,
                quoted,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewThread {
    path: ReviewPath,
    target: ThreadTarget,
    body: Body,
}

impl NewThread {
    #[must_use]
    pub fn new(path: ReviewPath, target: ThreadTarget, body: Body) -> Self {
        Self { path, target, body }
    }

    #[must_use]
    pub fn path(&self) -> &ReviewPath {
        &self.path
    }

    #[must_use]
    pub fn target(&self) -> &ThreadTarget {
        &self.target
    }

    #[must_use]
    pub fn body(&self) -> &Body {
        &self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::{line, range};

    #[test]
    fn without_a_start_line_the_thread_is_on_the_file() {
        assert_eq!(
            ThreadTarget::from_fields(None, None, None, None),
            Ok(ThreadTarget::File)
        );
    }

    #[test]
    fn line_fields_without_a_start_line_are_refused() {
        let refused = |end, side, quoted| ThreadTarget::from_fields(None, end, side, quoted);
        assert_eq!(
            refused(Some(line(2)), None, None),
            Err(TargetError::NeedsStartLine)
        );
        assert_eq!(
            refused(None, Some(Side::New), None),
            Err(TargetError::NeedsStartLine)
        );
        assert_eq!(
            refused(None, None, Some(Quote::new("q"))),
            Err(TargetError::NeedsStartLine)
        );
    }

    #[test]
    fn a_line_target_defaults_to_the_new_side_and_a_single_line() {
        assert_eq!(
            ThreadTarget::from_fields(Some(line(3)), None, None, None),
            Ok(ThreadTarget::Lines {
                side: Side::New,
                range: range(3, 3),
                quoted: None
            })
        );
        assert_eq!(
            ThreadTarget::from_fields(
                Some(line(1)),
                Some(line(2)),
                Some(Side::Old),
                Some(Quote::new("q"))
            ),
            Ok(ThreadTarget::Lines {
                side: Side::Old,
                range: range(1, 2),
                quoted: Some(Quote::new("q")),
            })
        );
    }

    #[test]
    fn an_end_line_before_the_start_is_refused() {
        assert_eq!(
            ThreadTarget::from_fields(Some(line(2)), Some(line(1)), None, None),
            Err(TargetError::Inverted(InvertedRange))
        );
    }

    #[test]
    fn a_new_thread_keeps_what_it_was_given() {
        let thread = NewThread::new(
            ReviewPath::parse("a.py").unwrap(),
            ThreadTarget::File,
            Body::parse("x").unwrap(),
        );
        assert_eq!(thread.path().as_str(), "a.py");
        assert_eq!(thread.target(), &ThreadTarget::File);
        assert_eq!(thread.body().as_str(), "x");
    }
}
