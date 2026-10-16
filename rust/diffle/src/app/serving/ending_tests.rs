use std::future::pending;
use std::time::Duration;

use super::*;

const GRACE: Duration = Duration::from_millis(30);
const PATIENCE: Duration = Duration::from_secs(10);
const LONG_ENOUGH_TO_HAVE_ENDED: Duration = Duration::from_millis(300);

async fn ending(browser: Browser, clients: watch::Receiver<usize>) -> Ending {
    let mut interrupts = Interrupts::listen().unwrap();
    wait_for_ending(&mut interrupts, browser, clients, GRACE, pending()).await
}

#[tokio::test]
async fn closing_the_last_opened_tab_ends_the_review() {
    let (count, clients) = watch::channel(0);
    // A page is connected before the review starts waiting; a watch keeps only the latest
    // value, so a page that came and went unseen would be indistinguishable from none.
    count.send_replace(1);
    let waiting = tokio::spawn(ending(Browser::EndsWithTab, clients));
    // Let the waiter see the page before it leaves.
    tokio::task::yield_now().await;
    count.send_replace(0);
    let ended = tokio::time::timeout(PATIENCE, waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(ended, Ending::LastTabClosed), "{ended:?}");
}

#[tokio::test]
async fn a_review_kept_alive_outlives_its_tab() {
    let (count, clients) = watch::channel(0);
    count.send_replace(1);
    count.send_replace(0);
    let outcome =
        tokio::time::timeout(LONG_ENOUGH_TO_HAVE_ENDED, ending(Browser::Kept, clients)).await;
    assert!(outcome.is_err(), "{outcome:?}");
}

#[tokio::test]
async fn a_review_nobody_opened_a_browser_for_outlives_the_tabs_too() {
    let (count, clients) = watch::channel(0);
    count.send_replace(1);
    count.send_replace(0);
    let outcome = tokio::time::timeout(
        LONG_ENOUGH_TO_HAVE_ENDED,
        ending(Browser::NotOpened, clients),
    )
    .await;
    assert!(outcome.is_err(), "{outcome:?}");
}

#[tokio::test]
async fn a_server_that_stops_ends_the_review_with_the_reason() {
    let (_count, clients) = watch::channel(0);
    let mut interrupts = Interrupts::listen().unwrap();
    let failure = std::future::ready(ServerError::Serve(std::io::Error::other("socket gone")));
    let ended = wait_for_ending(&mut interrupts, Browser::NotOpened, clients, GRACE, failure).await;
    assert!(
        matches!(&ended, Ending::ServerFailed(error) if error.to_string().contains("socket gone")),
        "{ended:?}"
    );
}
