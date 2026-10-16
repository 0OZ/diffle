//! Cutting a quote out of a file, and finding a quote again after the file changed.

use std::num::NonZeroUsize;

use crate::domains::comments::anchors::anchor::{LineAnchor, Quote};
use crate::domains::comments::anchors::lines::{Line, LineRange};

/// The lines of `contents`; a final newline does not start another line.
#[must_use]
pub fn split_lines(contents: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = contents.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

fn joined(lines: &[&str], range: LineRange) -> String {
    let count = range.end().index() + 1 - range.start().index();
    lines
        .iter()
        .skip(range.start().index())
        .take(count)
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Text of `range` joined with `\n`; lines past the end are simply missing.
#[must_use]
pub fn quote_lines(contents: &str, range: LineRange) -> String {
    joined(&split_lines(contents), range)
}

/// Text of `range`, or `None` when the range does not fit the contents: the server refuses to
/// quote lines that do not exist.
#[must_use]
pub fn quote_range(contents: &str, range: LineRange) -> Option<Quote> {
    let lines = split_lines(contents);
    if range.end().index() >= lines.len() {
        return None;
    }
    Some(Quote::new(joined(&lines, range)))
}

/// Finds the anchor's quote in `contents`, preferring the match nearest its start. Returns the
/// anchor moved onto the match, or `None` when the text is gone. An anchor that quotes nothing
/// cannot be lost.
#[must_use]
pub fn relocate(anchor: &LineAnchor, contents: &str) -> Option<LineAnchor> {
    let lines = split_lines(contents);
    let quoted = split_lines(anchor.quoted().as_str());
    let Some(span) = NonZeroUsize::new(quoted.len()) else {
        return Some(anchor.clone());
    };
    let wanted = anchor.range().start();
    let mut best: Option<(Line, u64)> = None;
    for (index, window) in lines.windows(span.get()).enumerate() {
        if window != quoted.as_slice() {
            continue;
        }
        let start = Line::at_index(index);
        let distance = start.distance(wanted);
        if best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((start, distance));
        }
        if distance == 0 {
            break;
        }
    }
    best.map(|(start, _)| anchor.moved_to(LineRange::spanning(start, span)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::{line, line_anchor, range};
    use crate::kernel::side::Side;

    fn anchor(start: u64, quoted: &str) -> LineAnchor {
        line_anchor("a.py", Side::New, start, quoted)
    }

    #[test]
    fn a_final_newline_does_not_start_a_line() {
        assert_eq!(split_lines("a\nb\n"), ["a", "b"]);
        assert_eq!(split_lines("a\n\n"), ["a", ""]);
        assert!(split_lines("").is_empty());
    }

    #[test]
    fn quote_lines_extracts_an_inclusive_one_based_range() {
        assert_eq!(quote_lines("a\nb\nc\nd\n", range(2, 3)), "b\nc");
        assert_eq!(quote_lines("a\n", range(2, 3)), "");
    }

    #[test]
    fn quote_range_refuses_lines_that_do_not_exist() {
        let quoted = |start, end| quote_range("a\nb\nc\n", range(start, end));
        assert_eq!(quoted(2, 3).unwrap().as_str(), "b\nc");
        assert_eq!(quoted(1, 3).unwrap().as_str(), "a\nb\nc");
        assert_eq!(quoted(3, 4), None);
        assert_eq!(quoted(4, 4), None);
    }

    #[test]
    fn relocate_keeps_an_anchor_whose_text_has_not_moved() {
        assert_eq!(relocate(&anchor(2, "b"), "a\nb\nc\n"), Some(anchor(2, "b")));
    }

    #[test]
    fn relocate_follows_text_that_shifted_down() {
        let moved = relocate(&anchor(2, "b\nc"), "x\ny\na\nb\nc\n").unwrap();
        assert_eq!(moved, anchor(4, "b\nc"));
        assert_eq!(moved.range(), range(4, 5));
    }

    #[test]
    fn relocate_prefers_the_match_nearest_the_original_line() {
        let contents = "x\nx\nx\nx\nx\nx\nx\nx\n";
        let start = |from| {
            relocate(&anchor(from, "x"), contents)
                .unwrap()
                .range()
                .start()
        };
        assert_eq!(start(6), line(6));
        assert_eq!(start(20), line(8));
    }

    #[test]
    fn relocate_returns_none_when_the_text_is_gone() {
        assert_eq!(relocate(&anchor(1, "gone"), "a\nb\n"), None);
    }

    #[test]
    fn relocate_requires_all_lines_of_a_block_to_match_contiguously() {
        assert_eq!(relocate(&anchor(1, "a\nc"), "a\nb\nc\n"), None);
    }

    #[test]
    fn an_anchor_that_quotes_nothing_stays_put() {
        let empty = LineAnchor::new(
            anchor(3, "x").path().clone(),
            Side::New,
            range(3, 3),
            Quote::new(""),
        );
        assert_eq!(relocate(&empty, "anything\n"), Some(empty));
    }
}
