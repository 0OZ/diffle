//! The lines of a side that a diff shows. A thread whose text survives outside them is stale.

use crate::domains::comments::anchors::lines::{Line, LineRange};
use crate::kernel::side::Side;

const HUNK_OPEN: &str = "@@ -";

/// Line ranges of one side that a unified patch shows: each hunk's span, context included. A
/// binary or empty patch shows nothing.
#[must_use]
pub fn shown_ranges(patch: &str, side: Side) -> Vec<LineRange> {
    patch
        .split('\n')
        .filter_map(parse_hunk_header)
        .filter_map(|(old, new)| {
            let (start, count) = match side {
                Side::Old => old,
                Side::New => new,
            };
            // A hunk of zero lines on this side (a pure addition, or deletion) shows none.
            let count = std::num::NonZeroUsize::new(usize::try_from(count).ok()?)?;
            Some(LineRange::spanning(Line::new(start).ok()?, count))
        })
        .collect()
}

/// Whether every line of `range` lies inside one shown range.
#[must_use]
pub fn is_shown(shown: &[LineRange], range: LineRange) -> bool {
    shown.iter().any(|candidate| candidate.encloses(range))
}

/// `@@ -a[,b] +c[,d] @@`: each side's start and line count (a missing count is one).
fn parse_hunk_header(line: &str) -> Option<((u64, u64), (u64, u64))> {
    let rest = line.strip_prefix(HUNK_OPEN)?;
    let (old, rest) = span(rest)?;
    let rest = rest.strip_prefix(" +")?;
    let (new, rest) = span(rest)?;
    rest.starts_with(" @@").then_some((old, new))
}

fn span(text: &str) -> Option<((u64, u64), &str)> {
    let (start, rest) = number(text)?;
    let Some(counted) = rest.strip_prefix(',') else {
        return Some(((start, 1), rest));
    };
    let (count, rest) = number(counted)?;
    Some(((start, count), rest))
}

fn number(text: &str) -> Option<(u64, &str)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    let (digits, rest) = text.split_at(digits);
    Some((digits.parse().ok()?, rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::range;

    const PATCH: &str = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,5 @@\n a\n+b\n c\n d\n e\n@@ -20 +21,2 @@ fn()\n x\n+y\n";

    #[test]
    fn reads_each_hunk_span_per_side_defaulting_an_omitted_count_to_one() {
        assert_eq!(shown_ranges(PATCH, Side::Old), [range(1, 4), range(20, 20)]);
        assert_eq!(shown_ranges(PATCH, Side::New), [range(1, 5), range(21, 22)]);
    }

    #[test]
    fn shows_nothing_for_an_empty_side_a_binary_patch_or_no_patch() {
        let added = "@@ -0,0 +1,2 @@\n+a\n+b\n";
        assert_eq!(shown_ranges(added, Side::Old), []);
        assert_eq!(shown_ranges(added, Side::New), [range(1, 2)]);
        assert_eq!(
            shown_ranges("Binary files a/x and b/x differ\n", Side::New),
            []
        );
        assert_eq!(shown_ranges("", Side::New), []);
    }

    #[test]
    fn ignores_lines_that_only_look_like_headers() {
        assert_eq!(
            shown_ranges("+@@ -1 +1 @@\n@@ -1 +1\n@@ -x +1 @@\n", Side::New),
            []
        );
        assert_eq!(shown_ranges("@@ -1,2 +3 @@", Side::New), [range(3, 3)]);
    }

    #[test]
    fn requires_the_whole_range_inside_one_hunk() {
        let shown = shown_ranges(PATCH, Side::New);
        assert!(is_shown(&shown, range(2, 5)));
        assert!(!is_shown(&shown, range(5, 6)));
        assert!(!is_shown(&shown, range(10, 10)));
        assert!(is_shown(&shown, range(22, 22)));
        assert!(!is_shown(&[], range(1, 1)));
    }
}
