//! Why a change to a review's comments is refused.

use thiserror::Error;

use super::anchors::lines::LineRange;
use super::anchors::review_path::ReviewPath;
use super::threads::ids::{MessageId, ThreadId};
use crate::kernel::side::Side;

/// A thread or message the caller named does not exist (the HTTP layer answers 404).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NotFound {
    #[error("no thread {0}")]
    Thread(ThreadId),
    #[error("no message {0}")]
    Message(MessageId),
}

/// An anchor the snapshot cannot place (the HTTP layer answers 400): a path absent on that
/// side, or lines past the end.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Unquotable {
    #[error("{0} is not in the review")]
    MissingFile(ReviewPath),
    #[error("cannot quote {path}:{range} on the {side} side")]
    Range {
        path: ReviewPath,
        side: Side,
        range: LineRange,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::anchors::lines::Line;

    #[test]
    fn errors_say_what_is_missing() {
        let id = ThreadId::from("t1".to_owned());
        assert_eq!(NotFound::Thread(id).to_string(), "no thread t1");
        let path = ReviewPath::parse("a.py").unwrap();
        let line = |n| Line::new(n).unwrap();
        assert_eq!(
            Unquotable::MissingFile(path.clone()).to_string(),
            "a.py is not in the review"
        );
        let range = LineRange::new(line(1), line(2)).unwrap();
        assert_eq!(
            Unquotable::Range {
                path,
                side: Side::Old,
                range
            }
            .to_string(),
            "cannot quote a.py:1-2 on the old side"
        );
    }
}
