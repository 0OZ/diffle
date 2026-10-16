//! The session reacting to the watcher: recomputing, announcing moved refs, retiring.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::session_fixtures::*;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::mode::mode_request::ModeRequest;
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::session::session_watch::WatchRequest;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::settings::follow_refs::FollowRefs;
#[tokio::test]
async fn a_worktree_change_recomputes_without_a_request() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit(".gitignore", "target/\n");
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;
    let mut events = session.subscribe();
    open(&session, &EntryRequest::Working).await;
    let WatchRequest::Worktree { ignored, root, .. } = fake.request() else {
        panic!("a working review watches the worktree");
    };
    assert_eq!(root, session.repo().root());
    assert!(!ignored.covers("a.txt"));
    write(&fixture, "target/out.o", "x");
    write(&fixture, "a.txt", "edited\n");

    fake.signal().send(()).await.unwrap();
    let SessionEvent::Snapshot(version) = next_event(&mut events).await else {
        panic!("a worktree change recomputes");
    };
    let snapshot = session
        .active()
        .unwrap()
        .snapshotter()
        .current()
        .await
        .unwrap();
    assert_eq!(snapshot.version(), version);
    assert_eq!(snapshot.parts().changed.len(), 1);
    // The ignored set is refreshed after the recompute, still inside the queue.
    session.close().await;
    assert!(ignored.covers("target/out.o"));
}

#[tokio::test]
async fn moved_refs_are_announced_and_a_reload_takes_them() {
    let (fixture, _) = branched();
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Off, Some(fake.factory())).await;
    let mut events = session.subscribe();
    let shown = open(&session, &revspec(&["main...feat"])).await;
    assert!(matches!(fake.request(), WatchRequest::Refs { .. }));

    fixture.commit("d.txt", "d\n");
    fake.signal().send(()).await.unwrap();
    let SessionEvent::Moved(Some(moved)) = next_event(&mut events).await else {
        panic!("the branch moved");
    };
    assert_eq!(moved.version, shown.version());
    assert_eq!(session.active().unwrap().moved(), Some(moved));
    assert_eq!(
        session
            .active()
            .unwrap()
            .snapshotter()
            .current()
            .await
            .unwrap()
            .version(),
        shown.version()
    );

    let reloaded = session.reload().await.unwrap();
    assert!(reloaded.version() > shown.version());
    assert_eq!(session.active().unwrap().moved(), None);
    assert_eq!(reloaded.parts().changed.len(), 3);
}

#[tokio::test]
async fn refs_that_come_back_clear_the_notice_and_unmoved_refs_say_nothing() {
    let (fixture, [_, tip]) = branched();
    let session = session(&fixture, FollowRefs::Off, None).await;
    let mut events = session.subscribe();
    open(&session, &revspec(&["main...feat"])).await;
    let active = session.active().unwrap();

    session.check_moved(&active).await;
    assert!(events.try_recv().is_err(), "nothing moved");

    fixture.commit("d.txt", "d\n");
    session.check_moved(&active).await;
    assert!(matches!(
        next_event(&mut events).await,
        SessionEvent::Moved(Some(_))
    ));
    session.check_moved(&active).await;
    assert!(
        events.try_recv().is_err(),
        "the same move is announced once"
    );

    fixture.git(&["reset", "--quiet", "--hard", tip.as_str()]);
    session.check_moved(&active).await;
    assert_eq!(next_event(&mut events).await, SessionEvent::Moved(None));
}

#[tokio::test]
async fn followed_refs_recompute_like_the_worktree() {
    let (fixture, _) = branched();
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::On, Some(fake.factory())).await;
    let mut events = session.subscribe();
    open(&session, &revspec(&["main...feat"])).await;
    fixture.commit("d.txt", "d\n");
    fake.signal().send(()).await.unwrap();
    assert!(matches!(
        next_event(&mut events).await,
        SessionEvent::Snapshot(_)
    ));
    assert_eq!(
        session
            .active()
            .unwrap()
            .snapshotter()
            .current()
            .await
            .unwrap()
            .parts()
            .changed
            .len(),
        3
    );
}

#[tokio::test]
async fn closing_stops_the_watch() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;
    open(&session, &EntryRequest::Working).await;
    assert!(!fake.stopped.load(Ordering::SeqCst));
    session.close().await;
    assert!(fake.stopped.load(Ordering::SeqCst));
    // The task that answers the watcher ends too, though the test still holds the sender.
    tokio::time::timeout(PATIENCE, fake.signal().closed())
        .await
        .unwrap();
}

#[tokio::test]
async fn switching_retires_the_previous_watch() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit("b.txt", "b\n");
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;
    open(&session, &EntryRequest::Working).await;
    // A pinned commit needs no watcher, and the working one's is stopped.
    session
        .switch_mode(ModeRequest::Enter(revspec(&["HEAD^!"])))
        .await
        .unwrap();
    assert!(fake.stopped.load(Ordering::SeqCst));
    assert_eq!(fake.started.lock().unwrap().len(), 1);
}

fn two_commits() -> TestRepo {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit("b.txt", "b\n");
    fixture
}

#[tokio::test]
async fn a_watcher_that_cannot_start_leaves_the_review_running() {
    let fixture = two_commits();
    let fake = Arc::new(FakeWatchers::default());
    fake.refuse.store(true, Ordering::SeqCst);
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;

    // Out of OS watches is no reason to refuse to show the diff.
    let shown = open(&session, &EntryRequest::Working).await;

    assert_eq!(shown.parts().changed.len(), 0);
    assert_eq!(
        session.active().unwrap().unwatched().as_deref(),
        Some("cannot watch for changes: no watches left")
    );
}

#[tokio::test]
async fn a_mode_switch_survives_a_watcher_that_cannot_start_and_the_next_one_tries_again() {
    let fixture = two_commits();
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;
    open(&session, &EntryRequest::Working).await;
    assert_eq!(session.active().unwrap().unwatched(), None);

    fake.refuse.store(true, Ordering::SeqCst);
    let switched = session
        .switch_mode(ModeRequest::Enter(revspec(&["HEAD~1..worktree"])))
        .await
        .unwrap();
    assert!(
        session
            .active()
            .unwrap()
            .mode()
            .range()
            .spec()
            .ends_at_worktree()
    );
    assert!(session.active().unwrap().unwatched().is_some());
    assert_eq!(switched.parts().changed.len(), 1);

    fake.refuse.store(false, Ordering::SeqCst);
    session
        .switch_mode(ModeRequest::Enter(EntryRequest::Working))
        .await
        .unwrap();
    assert_eq!(session.active().unwrap().unwatched(), None);
    assert_eq!(fake.started.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn the_previous_watch_is_stopped_before_the_next_one_starts() {
    let fixture = two_commits();
    let fake = Arc::new(FakeWatchers::default());
    let session = session(&fixture, FollowRefs::Auto, Some(fake.factory())).await;
    open(&session, &EntryRequest::Working).await;
    session
        .switch_mode(ModeRequest::Enter(revspec(&["HEAD~1..worktree"])))
        .await
        .unwrap();
    // Two live watchers at once would double the OS watches a switch needs.
    assert_eq!(*fake.stopped_when_asked.lock().unwrap(), [false, true]);
}
