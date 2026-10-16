//! A run of lines in a file, numbered the way editors and diffs number them.

use thiserror::Error;

/// Lines `start..=end`, 1-based. The fields are private so no span runs backwards or starts
/// before the first line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineSpan {
    start: u64,
    end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LineSpanError {
    #[error("lines are numbered from 1")]
    BeforeFirstLine,
    #[error("a line span does not end before it starts")]
    EndsBeforeStart,
}

impl LineSpan {
    pub const fn new(start: u64, end: u64) -> Result<Self, LineSpanError> {
        if start < 1 {
            return Err(LineSpanError::BeforeFirstLine);
        }
        if end < start {
            return Err(LineSpanError::EndsBeforeStart);
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }

    #[must_use]
    pub const fn contains(&self, line: u64) -> bool {
        self.start <= line && line <= self.end
    }
}

#[cfg(test)]
#[path = "line_span_tests.rs"]
mod tests;
