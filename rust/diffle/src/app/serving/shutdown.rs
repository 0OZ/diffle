//! Ending a run without waiting for work that may never finish: a Git command stuck on a dead
//! mount would otherwise keep Ctrl+C from ever taking effect.

use std::future::Future;
use std::time::Duration;

/// How long cleanup may take after the first signal. A recomputation in flight gets this long
/// to settle; Git children are killed when the process ends either way.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shutdown {
    Clean,
    /// Cleanup was still running when the grace period ended.
    GraceExpired,
    /// The reviewer signalled again: they want out now.
    Forced,
}

/// Runs `cleanup` until it finishes, `grace` passes, or `again` (the next signal) arrives.
pub async fn within_grace(
    grace: Duration,
    again: impl Future<Output = ()>,
    cleanup: impl Future<Output = ()>,
) -> Shutdown {
    tokio::select! {
        biased;
        () = cleanup => Shutdown::Clean,
        () = again => Shutdown::Forced,
        () = tokio::time::sleep(grace) => Shutdown::GraceExpired,
    }
}

#[cfg(test)]
#[path = "shutdown_tests.rs"]
mod tests;
