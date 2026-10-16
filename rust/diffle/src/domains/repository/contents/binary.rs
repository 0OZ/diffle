//! Git's rule for telling text from binary.

/// How far into a file Git looks for a NUL before calling it text.
pub const BINARY_SNIFF_BYTES: usize = 8000;

/// Whether `bytes` read as binary: a NUL within the first [`BINARY_SNIFF_BYTES`].
#[must_use]
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(BINARY_SNIFF_BYTES).any(|&byte| byte == 0)
}

#[cfg(test)]
#[path = "binary_tests.rs"]
mod tests;
