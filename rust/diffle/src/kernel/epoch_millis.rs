//! A point in time as the wire and the files on disk spell it: milliseconds since the Unix epoch.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochMillis(u64);

impl EpochMillis {
    #[must_use]
    pub const fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// A clock set before 1970 reads as the epoch itself.
    #[must_use]
    pub fn now() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        Self(u64::try_from(elapsed).unwrap_or(u64::MAX))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_after_2020_and_ordered() {
        const START_OF_2020: u64 = 1_577_836_800_000;
        let first = EpochMillis::now();
        assert!(first.get() > START_OF_2020);
        assert!(EpochMillis::now() >= first);
        assert_eq!(EpochMillis::new(7).get(), 7);
    }
}
