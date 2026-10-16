//! Mode transitions, focus, context and what a snapshot lets be read.

use std::sync::Arc;

use super::session_fixtures::*;
use crate::domains::comments::anchors::anchor::Anchor;
use crate::domains::comments::sample_threads::line_anchor;
use crate::domains::comments::threads::body::Body;
use crate::domains::comments::threads::thread_query::ThreadQuery;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::mode::mode_request::{FocusTarget, ModeRequest};
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;
use crate::kernel::side::Side;
#[tokio::test]
async fn nothing_is_open_before_the_first_mode() {
    let fixture = TestRepo::init();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    assert!(matches!(session.active(), Err(ReviewError::NotOpen)));
}

#[tokio::test]
async fn switching_announces_the_next_version_and_followers_see_it() {
    let (fixture, _) = branched();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    let first = open(&session, &EntryRequest::Working).await;
    assert_eq!(first.version().get(), 1);

    let mut events = session.subscribe();
    let followed = session.follow_snapshots();
    let second = session
        .switch_mode(ModeRequest::Enter(revspec(&["main...feat"])))
        .await
        .unwrap();
    assert_eq!(second.version().get(), 2);
    assert_eq!(
        next_event(&mut events).await,
        SessionEvent::Snapshot(second.version())
    );
    assert_eq!(
        followed.borrow().as_ref().map(|s| s.version()),
        Some(second.version())
    );
    assert_eq!(
        session
            .active()
            .unwrap()
            .mode()
            .comparison()
            .comment_key()
            .as_str(),
        "range:refs/heads/main...refs/heads/feat"
    );
}

#[tokio::test]
async fn a_rejected_switch_leaves_the_review_as_it_was() {
    let (fixture, _) = branched();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &EntryRequest::Working).await;
    let rejected = session
        .switch_mode(ModeRequest::Enter(revspec(&["nope..feat"])))
        .await;
    assert!(
        matches!(rejected, Err(ReviewError::Resolve(_))),
        "{rejected:?}"
    );
    let active = session.active().unwrap();
    assert_eq!(active.mode().comparison().comment_key().as_str(), "working");
    assert_eq!(
        active
            .snapshotter()
            .current()
            .await
            .unwrap()
            .version()
            .get(),
        1
    );
}

#[tokio::test]
async fn a_burst_of_refreshes_costs_one_recompute() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &EntryRequest::Working).await;
    write(&fixture, "a.txt", "edited\n");
    futures::future::join_all((0..20).map(|_| session.refresh())).await;
    let snapshot = session
        .active()
        .unwrap()
        .snapshotter()
        .current()
        .await
        .unwrap();
    assert_eq!(snapshot.version().get(), 2);
    assert_eq!(snapshot.parts().changed.len(), 1);
}

#[tokio::test]
async fn auto_follows_only_ranges_that_name_no_branch() {
    let (fixture, [first, second]) = branched();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &revspec(&["main...feat"])).await;
    assert!(!session.follows(&session.active().unwrap()));
    session
        .switch_mode(ModeRequest::Enter(revspec(&[
            first.as_str(),
            second.as_str(),
        ])))
        .await
        .unwrap();
    // Two hashes are pinned: nothing to follow either.
    assert!(!session.follows(&session.active().unwrap()));
    session
        .switch_mode(ModeRequest::Enter(EntryRequest::Working))
        .await
        .unwrap();
    assert!(
        session.follows(&session.active().unwrap()),
        "the worktree always follows"
    );
}

#[tokio::test]
async fn focusing_a_listed_commit_keeps_the_range_and_its_own_comments() {
    let (fixture, [first, _]) = branched();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &revspec(&["main...feat"])).await;

    let focused = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Commit(
            first.as_str()[..7].to_owned(),
        )))
        .await
        .unwrap();
    assert_eq!(
        focused.parts().commit.as_ref().map(|c| c.info.id.clone()),
        Some(first.clone())
    );
    assert_eq!(
        focused.parts().commits.commits.total,
        2,
        "the range still lists its commits"
    );
    let mode = session.active().unwrap().mode().clone();
    assert_eq!(
        mode.comparison().comment_key().as_str(),
        format!("commit:{first}")
    );
    assert_eq!(
        mode.within().map(|range| range.comment_key().as_str()),
        Some("range:refs/heads/main...refs/heads/feat")
    );

    let back = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Range))
        .await
        .unwrap();
    assert!(back.parts().commit.is_none());
    assert_eq!(back.parts().changed.len(), 2);
}

#[tokio::test]
async fn focus_refuses_what_the_range_cannot_show() {
    let (fixture, _) = branched();
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &revspec(&["main...feat"])).await;
    let unlisted = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Commit(
            "abcdef0".to_owned(),
        )))
        .await;
    assert_eq!(
        unlisted.unwrap_err().to_string(),
        "not a listed commit of main...feat: abcdef0"
    );
    let no_changes = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Worktree))
        .await;
    assert_eq!(
        no_changes.unwrap_err().to_string(),
        "main...feat has no uncommitted changes"
    );

    session
        .switch_mode(ModeRequest::Enter(revspec(&["HEAD^!"])))
        .await
        .unwrap();
    let alone = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Range))
        .await;
    assert!(matches!(alone, Err(ReviewError::NothingToFocus)));
}

#[tokio::test]
async fn the_uncommitted_changes_of_a_range_share_the_working_comments() {
    let (fixture, _) = branched();
    write(&fixture, "c.txt", "edited\n");
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &revspec(&["main..worktree"])).await;
    let focused = session
        .switch_mode(ModeRequest::Focus(FocusTarget::Worktree))
        .await
        .unwrap();
    assert_eq!(focused.parts().changed.len(), 1);
    assert_eq!(
        session
            .active()
            .unwrap()
            .mode()
            .comparison()
            .comment_key()
            .as_str(),
        "working"
    );
    assert_eq!(
        focused.parts().commits.commits.total,
        2,
        "the range's commits stay listed"
    );
}

#[tokio::test]
async fn a_new_context_recomputes_and_the_same_one_does_not() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let session = session(&fixture, FollowRefs::Auto, None).await;
    let mut events = session.subscribe();
    open(&session, &EntryRequest::Working).await;
    session.set_context(ContextLines::default()).await.unwrap();
    assert!(events.try_recv().is_err());
    let wider = ContextLines::new(20.0).unwrap();
    session.set_context(wider).await.unwrap();
    assert_eq!(
        next_event(&mut events).await,
        SessionEvent::Snapshot(
            session
                .active()
                .unwrap()
                .snapshotter()
                .current()
                .await
                .unwrap()
                .version()
        )
    );
    assert_eq!(session.context(), wider);
    assert_eq!(
        session
            .active()
            .unwrap()
            .snapshotter()
            .current()
            .await
            .unwrap()
            .parts()
            .context,
        wider
    );
}

#[tokio::test]
async fn only_the_snapshots_files_can_be_read() {
    let fixture = TestRepo::init();
    fixture.commit("old.txt", "one\ntwo\nthree\nfour\nfive\n");
    fixture.commit(".gitignore", "secret.txt\n");
    fixture.git(&["mv", "old.txt", "new.txt"]);
    write(&fixture, "new.txt", "one\ntwo\nthree\nfour\nfive\nsix\n");
    write(&fixture, "secret.txt", "hunter2");
    let session = session(&fixture, FollowRefs::Auto, None).await;
    let snapshot = open(&session, &EntryRequest::Working).await;
    let read = |name: &'static str, side| {
        let (session, snapshot) = (Arc::clone(&session), Arc::clone(&snapshot));
        async move {
            session
                .read_side(&snapshot, &path(name), side)
                .await
                .unwrap()
        }
    };
    assert_eq!(
        read("new.txt", Side::New).await.unwrap(),
        b"one\ntwo\nthree\nfour\nfive\nsix\n"
    );
    assert_eq!(
        read("new.txt", Side::Old).await.unwrap(),
        b"one\ntwo\nthree\nfour\nfive\n"
    );
    assert_eq!(
        read("secret.txt", Side::New).await,
        None,
        "ignored files are not exposed"
    );
    assert_eq!(read(".git/config", Side::New).await, None);
    assert_eq!(read("missing.txt", Side::New).await, None);
}

#[tokio::test]
async fn comments_follow_their_lines_when_the_file_changes() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "one\ntwo\nthree\n");
    write(&fixture, "a.txt", "one\ntwo changed\nthree\n");
    let session = session(&fixture, FollowRefs::Auto, None).await;
    open(&session, &EntryRequest::Working).await;
    let comments = session.active().unwrap().comments();
    let anchor = Anchor::Line(line_anchor("a.txt", Side::New, 2, "two changed"));
    let thread = comments
        .add_thread(anchor, Body::parse("why?").unwrap())
        .await
        .unwrap();

    write(&fixture, "a.txt", "zero\none\ntwo changed\nthree\n");
    session.refresh().await;
    let moved = comments.get(thread.id()).await.unwrap().unwrap();
    let Anchor::Line(anchor) = moved.anchor() else {
        panic!("a line thread")
    };
    assert_eq!(anchor.range().start().index(), 2);
    assert!(!moved.is_stale());

    write(&fixture, "a.txt", "gone entirely\n");
    session.refresh().await;
    let lost = comments.threads(&ThreadQuery::all()).await.unwrap();
    assert!(lost[0].is_stale());
}
