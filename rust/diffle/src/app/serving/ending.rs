//! What ends a review: the reviewer, the tab that was opened for it, or the server failing.

use std::future::Future;
use std::time::Duration;

use tokio::sync::watch;

use crate::app::config::Browser;
use crate::workers::http_server::pages::browser_lifetime::last_tab_closed;
use crate::workers::http_server::server_error::ServerError;
use crate::workers::interrupt::Interrupts;

#[derive(Debug)]
pub enum Ending {
    /// Ctrl+C or the termination signal.
    Interrupted,
    /// The last page the review opened went away and stayed away.
    LastTabClosed,
    ServerFailed(ServerError),
}

/// Resolves at the first ending. A tab closing counts only when the browser was opened to end
/// the review with it; `tab_grace` is how long a reload may take before the tab is "gone".
pub async fn wait_for_ending(
    interrupts: &mut Interrupts,
    browser: Browser,
    clients: watch::Receiver<usize>,
    tab_grace: Duration,
    server_failed: impl Future<Output = ServerError>,
) -> Ending {
    tokio::select! {
        () = interrupts.wait() => Ending::Interrupted,
        () = tab_closed(browser, clients, tab_grace) => Ending::LastTabClosed,
        error = server_failed => Ending::ServerFailed(error),
    }
}

async fn tab_closed(browser: Browser, clients: watch::Receiver<usize>, grace: Duration) {
    match browser.ends_with_tab() {
        true => last_tab_closed(clients, grace).await,
        false => std::future::pending().await,
    }
}

#[cfg(test)]
#[path = "ending_tests.rs"]
mod tests;
