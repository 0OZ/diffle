//! How many unchanged lines surround each change (`git diff -U`).

use thiserror::Error;

const MAX: u32 = 10_000;
const DEFAULT: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLines(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("contextLines must be a whole number from 0 to {MAX}")]
pub struct ContextLinesError;

impl ContextLines {
    /// A value from a request: refused when it is not a whole number in range.
    pub fn new(value: f64) -> Result<Self, ContextLinesError> {
        match value.fract() == 0.0 && (0.0..=f64::from(MAX)).contains(&value) {
            // Checked whole and in range just above, so the cast is exact.
            true => Ok(Self(value as u32)),
            false => Err(ContextLinesError),
        }
    }

    /// A value from a file: anything unusable falls back to the default, and a fraction or an
    /// excess is clamped, so a hand-edited config never stops the server.
    #[must_use]
    pub fn normalized(value: f64) -> Self {
        if !value.is_finite() {
            return Self::default();
        }
        Self(value.floor().clamp(0.0, f64::from(MAX)) as u32)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl Default for ContextLines {
    fn default() -> Self {
        Self(DEFAULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_must_be_a_whole_number_in_range() {
        assert_eq!(ContextLines::new(0.0).unwrap().get(), 0);
        assert_eq!(ContextLines::new(10_000.0).unwrap().get(), 10_000);
        for bad in [-1.0, 10_001.0, 2.5, f64::NAN, f64::INFINITY] {
            assert_eq!(ContextLines::new(bad), Err(ContextLinesError), "{bad}");
        }
    }

    #[test]
    fn a_file_value_is_clamped_or_defaulted() {
        assert_eq!(ContextLines::normalized(3.9).get(), 3);
        assert_eq!(ContextLines::normalized(-4.0).get(), 0);
        assert_eq!(ContextLines::normalized(1e9).get(), 10_000);
        assert_eq!(ContextLines::normalized(f64::NAN), ContextLines::default());
        assert_eq!(ContextLines::default().get(), 5);
    }
}
