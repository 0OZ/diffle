use std::future::pending;
use std::time::Duration;

use super::*;

const GRACE: Duration = Duration::from_millis(80);
const NEVER: Duration = Duration::from_secs(3600);

#[tokio::test]
async fn cleanup_that_finishes_in_time_is_clean() {
    let outcome = within_grace(NEVER, pending(), async {}).await;
    assert_eq!(outcome, Shutdown::Clean);
}

#[tokio::test]
async fn cleanup_that_never_finishes_is_cut_off_when_the_grace_ends() {
    let started = tokio::time::Instant::now();
    let outcome = within_grace(GRACE, pending(), pending()).await;
    assert_eq!(outcome, Shutdown::GraceExpired);
    assert!(started.elapsed() >= GRACE);
}

#[tokio::test]
async fn another_signal_ends_the_wait_before_the_grace_does() {
    let started = tokio::time::Instant::now();
    let outcome = within_grace(NEVER, async {}, pending()).await;
    assert_eq!(outcome, Shutdown::Forced);
    assert!(started.elapsed() < NEVER);
}

#[tokio::test]
async fn cleanup_that_is_already_done_wins_over_a_signal_that_arrives_with_it() {
    let outcome = within_grace(NEVER, async {}, async {}).await;
    assert_eq!(outcome, Shutdown::Clean);
}
