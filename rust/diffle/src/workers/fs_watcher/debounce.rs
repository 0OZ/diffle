//! The quiet-period clock: every change restarts it, and it is due once nothing changed
//! for the whole period.

use std::time::Duration;

use tokio::time::Instant;

/// Longest quiet period honoured. It keeps `now + quiet` far from overflowing `Instant`.
const MAX_QUIET: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone)]
pub struct Debouncer {
    quiet: Duration,
    deadline: Option<Instant>,
}

impl Debouncer {
    /// A period beyond [`MAX_QUIET`] is cut to it.
    #[must_use]
    pub fn new(quiet: Duration) -> Self {
        Self {
            quiet: quiet.min(MAX_QUIET),
            deadline: None,
        }
    }

    /// A change happened at `now`: the signal is due one quiet period from now.
    pub fn touch(&mut self, now: Instant) {
        self.deadline = Some(now + self.quiet);
    }

    /// When the pending signal is due, or `None` when no change is waiting.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// The pending signal was delivered.
    pub fn settle(&mut self) {
        self.deadline = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_due_before_a_change() {
        assert_eq!(Debouncer::new(Duration::from_millis(30)).deadline(), None);
    }

    #[test]
    fn every_change_pushes_the_deadline_out() {
        let mut debouncer = Debouncer::new(Duration::from_millis(30));
        let start = Instant::now();
        debouncer.touch(start);
        assert_eq!(
            debouncer.deadline(),
            Some(start + Duration::from_millis(30))
        );
        let later = start + Duration::from_millis(20);
        debouncer.touch(later);
        assert_eq!(
            debouncer.deadline(),
            Some(later + Duration::from_millis(30))
        );
    }

    #[test]
    fn settling_clears_the_pending_signal() {
        let mut debouncer = Debouncer::new(Duration::from_millis(30));
        debouncer.touch(Instant::now());
        debouncer.settle();
        assert_eq!(debouncer.deadline(), None);
    }

    #[test]
    fn an_absurd_period_is_cut_instead_of_overflowing() {
        let mut debouncer = Debouncer::new(Duration::MAX);
        let now = Instant::now();
        debouncer.touch(now);
        assert_eq!(debouncer.deadline(), Some(now + MAX_QUIET));
    }
}
