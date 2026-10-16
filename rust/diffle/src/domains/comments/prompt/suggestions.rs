//! Splitting a message body into prose and suggestion fences.

use crate::domains::comments::threads::body::trimmed;

const OPENING_FENCE: &str = "```suggestion";
const CLOSING_FENCE: &str = "```";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodySegment {
    /// Prose before or between suggestions; may be blank.
    Prose(String),
    /// Replacement code for the quoted range, from a ```suggestion fence.
    Suggestion(String),
}

/// Splits a body into prose and ```suggestion blocks, in order. Plain text yields one segment.
///
/// A fence opens at a line that starts with ```suggestion (anything may follow on that line) and
/// closes at the next line that is only ``` and optional blanks. A fence that never closes is
/// prose.
#[must_use]
pub fn parse_suggestions(body: &str) -> Vec<BodySegment> {
    let mut segments = Vec::new();
    let mut last = 0;
    while let Some(fence) = next_fence(body, last) {
        let before = &body[last..fence.start];
        if !trimmed(before).is_empty() {
            segments.push(BodySegment::Prose(before.to_owned()));
        }
        segments.push(BodySegment::Suggestion(fence.code.to_owned()));
        last = fence.end;
    }
    let rest = &body[last..];
    if !trimmed(rest).is_empty() || segments.is_empty() {
        segments.push(BodySegment::Prose(rest.to_owned()));
    }
    segments
}

struct Fence<'a> {
    start: usize,
    end: usize,
    code: &'a str,
}

/// The first complete fence opening at or after `from`.
fn next_fence(body: &str, from: usize) -> Option<Fence<'_>> {
    body[from..]
        .match_indices(OPENING_FENCE)
        .map(|(offset, _)| from + offset)
        .filter(|&start| starts_line(body, start))
        .find_map(|start| close_fence(body, start))
}

fn close_fence(body: &str, start: usize) -> Option<Fence<'_>> {
    let after_opening = start + OPENING_FENCE.len();
    let code_start = after_opening + body[after_opening..].find('\n')? + 1;
    // The closing fence is the first fence line from `code_start`; the newline before it is not
    // part of the code, and a fence right at `code_start` closes an empty suggestion.
    let ends = body[code_start..]
        .char_indices()
        .map(|(offset, ch)| (code_start + offset, Some(ch)))
        .chain([(body.len(), None)]);
    for (at, ch) in ends {
        let fence_line = match ch {
            Some('\n') => at + 1,
            _ if starts_line(body, at) => at,
            _ => continue,
        };
        if let Some(end) = closing_fence_end(body, fence_line) {
            return Some(Fence {
                start,
                end,
                code: &body[code_start..at],
            });
        }
    }
    None
}

/// The end of a line at `at` that holds only a closing fence and blanks.
fn closing_fence_end(body: &str, at: usize) -> Option<usize> {
    let after_fence = body[at..].strip_prefix(CLOSING_FENCE)?;
    let rest = after_fence.trim_start_matches([' ', '\t']);
    let ends_line = rest.chars().next().is_none_or(is_line_end);
    ends_line.then_some(body.len() - rest.len())
}

fn starts_line(body: &str, at: usize) -> bool {
    body[..at].chars().next_back().is_none_or(is_line_end)
}

/// The line terminators JavaScript's multiline `^` and `$` know.
fn is_line_end(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose(text: &str) -> BodySegment {
        BodySegment::Prose(text.to_owned())
    }

    fn suggestion(code: &str) -> BodySegment {
        BodySegment::Suggestion(code.to_owned())
    }

    #[test]
    fn splits_prose_and_fences_in_order_keeps_plain_text_whole_and_accepts_an_empty_suggestion() {
        assert_eq!(parse_suggestions("just text"), [prose("just text")]);
        assert_eq!(parse_suggestions("```suggestion\n```"), [suggestion("")]);
        assert_eq!(
            parse_suggestions("a\n```suggestion\nx\ny\n```\nb\n```suggestion\nz\n```"),
            [
                prose("a\n"),
                suggestion("x\ny"),
                prose("\nb\n"),
                suggestion("z")
            ]
        );
        // A regular code fence is prose.
        assert_eq!(
            parse_suggestions("```python\nx\n```"),
            [prose("```python\nx\n```")]
        );
    }

    #[test]
    fn a_fence_needs_a_line_to_itself_on_both_ends() {
        assert_eq!(
            parse_suggestions("see ```suggestion\nx\n```"),
            [prose("see ```suggestion\nx\n```")]
        );
        assert_eq!(
            parse_suggestions("```suggestion\nx\n``` trailing"),
            [prose("```suggestion\nx\n``` trailing")]
        );
        assert_eq!(
            parse_suggestions("```suggestion\nx"),
            [prose("```suggestion\nx")]
        );
        assert_eq!(parse_suggestions("```suggestion"), [prose("```suggestion")]);
    }

    #[test]
    fn the_opening_line_may_carry_a_note_and_the_closing_line_blanks() {
        assert_eq!(
            parse_suggestions("```suggestion note\nx\n```  \t"),
            [suggestion("x")]
        );
        assert_eq!(
            parse_suggestions("```suggestion\r\nx\r\n```\r\n"),
            [suggestion("x\r")]
        );
    }

    #[test]
    fn blank_prose_around_fences_is_dropped_but_a_blank_body_is_kept() {
        assert_eq!(
            parse_suggestions("\n```suggestion\nx\n```\n  \n"),
            [suggestion("x")]
        );
        assert_eq!(parse_suggestions(""), [prose("")]);
    }

    #[test]
    fn the_first_closing_fence_wins() {
        assert_eq!(
            parse_suggestions("```suggestion\na\n```\nb\n```"),
            [suggestion("a"), prose("\nb\n```")]
        );
    }
}
