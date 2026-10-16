//! What a batch of new threads must ask of the snapshot, and what the snapshot answered.
//!
//! Placing a thread needs the review's files, which the store must not wait on while it holds
//! its lock. So the caller asks first ([`AnchorQuestions::of`]), looks everything up, and hands
//! the answers to the import ([`AnchorAnswers`]). The store consults an answer only for a
//! request that is not a duplicate, so asking eagerly never turns a duplicate into an error.

use std::collections::{HashMap, HashSet};

use super::new_thread::{NewThread, ThreadTarget};
use crate::domains::comments::anchors::anchor::{Anchor, LineAnchor, Quote};
use crate::domains::comments::anchors::lines::LineRange;
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::comment_error::Unquotable;
use crate::kernel::side::Side;

/// The exact text of a range on one side of a path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QuoteQuestion {
    pub path: ReviewPath,
    pub side: Side,
    pub range: LineRange,
}

/// The lookups a batch needs, without repeats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnchorQuestions {
    quotes: Vec<QuoteQuestion>,
    files: Vec<ReviewPath>,
}

impl AnchorQuestions {
    #[must_use]
    pub fn of(requests: &[NewThread]) -> Self {
        let mut questions = Self::default();
        for request in requests {
            match request.target() {
                ThreadTarget::File => {
                    if !questions.files.contains(request.path()) {
                        questions.files.push(request.path().clone());
                    }
                }
                ThreadTarget::Lines { side, range, .. } => {
                    let question = QuoteQuestion {
                        path: request.path().clone(),
                        side: *side,
                        range: *range,
                    };
                    if !questions.quotes.contains(&question) {
                        questions.quotes.push(question);
                    }
                }
            }
        }
        questions
    }

    /// Ranges to quote from the snapshot, behind its allowlist.
    #[must_use]
    pub fn quotes(&self) -> &[QuoteQuestion] {
        &self.quotes
    }

    /// Paths whose presence in the review must be confirmed.
    #[must_use]
    pub fn files(&self) -> &[ReviewPath] {
        &self.files
    }
}

/// The snapshot's answers. A question not answered, or answered with nothing, means the anchor
/// cannot be placed.
#[derive(Debug, Clone, Default)]
pub struct AnchorAnswers {
    quotes: HashMap<QuoteQuestion, Quote>,
    files: HashSet<ReviewPath>,
}

impl AnchorAnswers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The range exists, and reads as `quote`.
    pub fn record_quote(&mut self, question: QuoteQuestion, quote: Quote) {
        self.quotes.insert(question, quote);
    }

    /// The review has a file at `path` on either side.
    pub fn record_file(&mut self, path: ReviewPath) {
        self.files.insert(path);
    }

    /// The anchor `request` lands on: a range is quoted (a payload's own `quoted` is kept once
    /// the range is known to exist) and a file thread's path must be in the review.
    pub fn place(&self, request: &NewThread) -> Result<Anchor, Unquotable> {
        let path = request.path();
        match request.target() {
            ThreadTarget::File => {
                if !self.files.contains(path) {
                    return Err(Unquotable::MissingFile(path.clone()));
                }
                Ok(Anchor::File(path.clone()))
            }
            ThreadTarget::Lines {
                side,
                range,
                quoted,
            } => {
                let question = QuoteQuestion {
                    path: path.clone(),
                    side: *side,
                    range: *range,
                };
                let Some(from_snapshot) = self.quotes.get(&question) else {
                    return Err(Unquotable::Range {
                        path: path.clone(),
                        side: *side,
                        range: *range,
                    });
                };
                let quote = quoted.clone().unwrap_or_else(|| from_snapshot.clone());
                Ok(Anchor::Line(LineAnchor::new(
                    path.clone(),
                    *side,
                    *range,
                    quote,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::{path, range};
    use crate::domains::comments::threads::body::Body;

    fn lines(file: &str, start: u64, end: u64, quoted: Option<&str>) -> NewThread {
        let target = ThreadTarget::Lines {
            side: Side::New,
            range: range(start, end),
            quoted: quoted.map(Quote::new),
        };
        NewThread::new(path(file), target, Body::parse("x").unwrap())
    }

    fn whole_file(file: &str) -> NewThread {
        NewThread::new(path(file), ThreadTarget::File, Body::parse("x").unwrap())
    }

    #[test]
    fn a_batch_asks_each_question_once() {
        let batch = [
            lines("a.py", 1, 1, None),
            lines("a.py", 1, 1, Some("given")),
            lines("a.py", 2, 3, None),
            whole_file("a.py"),
            whole_file("a.py"),
        ];
        let questions = AnchorQuestions::of(&batch);
        assert_eq!(questions.quotes().len(), 2);
        assert_eq!(questions.files(), [path("a.py")]);
    }

    #[test]
    fn a_range_is_quoted_from_the_snapshot_unless_the_payload_brought_its_own() {
        let mut answers = AnchorAnswers::new();
        let question = QuoteQuestion {
            path: path("a.py"),
            side: Side::New,
            range: range(1, 1),
        };
        answers.record_quote(question, Quote::new("L1-1"));
        let placed = |request| answers.place(&request);
        let Ok(Anchor::Line(quoted)) = placed(lines("a.py", 1, 1, None)) else {
            panic!("the range is known");
        };
        assert_eq!(quoted.quoted().as_str(), "L1-1");
        let Ok(Anchor::Line(given)) = placed(lines("a.py", 1, 1, Some("as shown"))) else {
            panic!("the range is known");
        };
        assert_eq!(given.quoted().as_str(), "as shown");
    }

    #[test]
    fn an_unanswered_range_or_file_cannot_be_placed() {
        let mut answers = AnchorAnswers::new();
        answers.record_file(path("a.py"));
        assert_eq!(
            answers.place(&lines("a.py", 8, 8, Some("root:x:0:0"))),
            Err(Unquotable::Range {
                path: path("a.py"),
                side: Side::New,
                range: range(8, 8)
            })
        );
        assert_eq!(
            answers.place(&whole_file("a.py")),
            Ok(Anchor::File(path("a.py")))
        );
        assert_eq!(
            answers.place(&whole_file("gone.py")),
            Err(Unquotable::MissingFile(path("gone.py")))
        );
    }
}
