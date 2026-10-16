use std::time::Duration;

use super::*;

const GRACE: Duration = Duration::from_millis(60);
const LONGER: Duration = Duration::from_millis(250);

fn watching() -> (watch::Sender<usize>, tokio::task::JoinHandle<()>) {
    let (sender, receiver) = watch::channel(0);
    (sender, tokio::spawn(last_tab_closed(receiver, GRACE)))
}

#[tokio::test]
async fn nothing_ends_before_a_page_has_connected() {
    let (_clients, task) = watching();
    assert!(tokio::time::timeout(LONGER, task).await.is_err());
}

#[tokio::test]
async fn the_review_ends_when_the_last_page_stays_gone() {
    let (clients, task) = watching();
    clients.send_replace(1);
    tokio::time::sleep(GRACE * 2).await;
    assert!(!task.is_finished(), "a connected page keeps it open");
    clients.send_replace(0);
    tokio::time::timeout(LONGER, task).await.unwrap().unwrap();
}

#[tokio::test]
async fn a_reload_within_the_grace_period_keeps_it_open() {
    let (clients, task) = watching();
    clients.send_replace(1);
    clients.send_replace(0);
    tokio::time::sleep(GRACE / 3).await;
    clients.send_replace(1);
    assert!(tokio::time::timeout(LONGER, task).await.is_err());
}

#[tokio::test]
async fn a_second_page_closing_does_not_end_it_while_one_remains() {
    let (clients, task) = watching();
    clients.send_replace(2);
    clients.send_replace(1);
    assert!(tokio::time::timeout(LONGER, task).await.is_err());
}

#[tokio::test]
async fn a_server_that_is_gone_ends_the_wait() {
    let (clients, task) = watching();
    drop(clients);
    tokio::time::timeout(LONGER, task).await.unwrap().unwrap();
}
