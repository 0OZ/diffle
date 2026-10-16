//! A review that opened its own browser ends when that browser goes away.

use std::time::Duration;

use tokio::sync::watch;

/// How long the last page may be gone before the review ends: a reload replaces its socket
/// within this time without ending anything.
pub const CLOSE_GRACE: Duration = Duration::from_secs(1);

/// Completes once a page has connected and then every page stays gone for `grace`. Also
/// completes if the server that counts the pages is gone.
pub async fn last_tab_closed(mut clients: watch::Receiver<usize>, grace: Duration) {
    if clients.wait_for(|count| *count > 0).await.is_err() {
        return;
    }
    loop {
        if clients.wait_for(|count| *count == 0).await.is_err() {
            return;
        }
        match tokio::time::timeout(grace, clients.wait_for(|count| *count > 0)).await {
            // Gone for the whole grace period, or nobody left to count.
            Err(_) | Ok(Err(_)) => return,
            Ok(Ok(_back)) => {}
        }
    }
}

#[cfg(test)]
#[path = "browser_lifetime_tests.rs"]
mod tests;
