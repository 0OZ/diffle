//! Ctrl+C and the termination signal: the two ways the person who started a review ends it.
//!
//! Tested in `tests/interrupt.rs`: a signal reaches the whole process, so it cannot be sent
//! among tests that share one.

use std::io;

#[cfg(unix)]
use tokio::signal::unix::{Signal, SignalKind, signal};

/// Registered handlers for the signals that end a review. Registered on creation, so a signal
/// that arrives before [`wait`](Self::wait) is polled is not lost.
pub struct Interrupts {
    #[cfg(unix)]
    terminate: Signal,
}

impl Interrupts {
    pub fn listen() -> io::Result<Self> {
        Ok(Self {
            #[cfg(unix)]
            terminate: signal(SignalKind::terminate())?,
        })
    }

    /// Completes at the first Ctrl+C or termination signal.
    pub async fn wait(&mut self) {
        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = self.terminate.recv() => {}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    }
}
