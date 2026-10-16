//! One-based line numbers, and the ranges they bound.

use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Line(NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("line numbers start at 1")]
pub struct ZeroLine;

impl Line {
    pub fn new(number: u64) -> Result<Self, ZeroLine> {
        NonZeroU64::new(number).map(Self).ok_or(ZeroLine)
    }

    /// The line at a zero-based position in a list of lines.
    #[must_use]
    pub fn at_index(index: usize) -> Self {
        let number = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        Self(NonZeroU64::new(number).unwrap_or(NonZeroU64::MAX))
    }

    /// The zero-based position of this line in a list of lines.
    #[must_use]
    pub fn index(self) -> usize {
        usize::try_from(self.get() - 1).unwrap_or(usize::MAX)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub fn distance(self, other: Self) -> u64 {
        self.get().abs_diff(other.get())
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(f)
    }
}

/// An inclusive range of lines. The end never precedes the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineRange {
    start: Line,
    end: Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a line range cannot end before it starts")]
pub struct InvertedRange;

impl LineRange {
    pub fn new(start: Line, end: Line) -> Result<Self, InvertedRange> {
        if end < start {
            return Err(InvertedRange);
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn single(line: Line) -> Self {
        Self {
            start: line,
            end: line,
        }
    }

    /// The `len` lines from `start` on.
    #[must_use]
    pub fn spanning(start: Line, len: NonZeroUsize) -> Self {
        let extra = u64::try_from(len.get() - 1).unwrap_or(u64::MAX);
        let end = Line::new(start.get().saturating_add(extra)).unwrap_or(start);
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> Line {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Line {
        self.end
    }

    /// Whether every line of `inner` lies inside this range.
    #[must_use]
    pub fn encloses(self, inner: Self) -> bool {
        inner.start >= self.start && inner.end <= self.end
    }
}

/// `3` for one line, `3-4` for several: how a range reads in a prompt.
impl fmt::Display for LineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            self.start.fmt(f)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(number: u64) -> Line {
        Line::new(number).unwrap()
    }

    #[test]
    fn lines_start_at_one_and_convert_to_indexes() {
        assert_eq!(Line::new(0), Err(ZeroLine));
        assert_eq!(Line::at_index(0), line(1));
        assert_eq!(line(5).index(), 4);
        assert_eq!(line(5).get(), 5);
        assert_eq!(line(3).distance(line(8)), 5);
        assert_eq!(line(8).distance(line(3)), 5);
    }

    #[test]
    fn a_range_cannot_be_inverted() {
        assert_eq!(LineRange::new(line(3), line(2)), Err(InvertedRange));
        let range = LineRange::new(line(2), line(4)).unwrap();
        assert_eq!((range.start(), range.end()), (line(2), line(4)));
        assert_eq!(LineRange::single(line(2)).end(), line(2));
    }

    #[test]
    fn spanning_counts_the_lines_it_covers() {
        let len = NonZeroUsize::new(3).unwrap();
        assert_eq!(
            LineRange::spanning(line(4), len),
            LineRange::new(line(4), line(6)).unwrap()
        );
        let one = NonZeroUsize::MIN;
        assert_eq!(
            LineRange::spanning(line(4), one),
            LineRange::single(line(4))
        );
    }

    #[test]
    fn enclosing_needs_the_whole_inner_range() {
        let outer = LineRange::new(line(1), line(5)).unwrap();
        let inside = LineRange::new(line(2), line(5)).unwrap();
        let straddling = LineRange::new(line(5), line(6)).unwrap();
        assert!(outer.encloses(inside));
        assert!(!outer.encloses(straddling));
        assert!(!inside.encloses(outer));
    }

    #[test]
    fn a_range_reads_as_one_line_or_a_span() {
        assert_eq!(LineRange::single(line(3)).to_string(), "3");
        assert_eq!(LineRange::new(line(3), line(4)).unwrap().to_string(), "3-4");
    }
}
