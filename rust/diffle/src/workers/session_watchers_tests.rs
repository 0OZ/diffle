use std::time::Duration;

use super::*;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::session::review_session::{Session, SessionConfig};
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

const PATIENCE: Duration = Duration::from_secs(10);

#[tokio::test]
async fn a_session_recomputes_when_a_file_in_its_worktree_changes() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let repo = std::sync::Arc::new(GitRepo::open(fixture.path()).await.unwrap());
    let session = Session::new(
        repo,
        SessionConfig {
            context: ContextLines::default(),
            follow_refs: FollowRefs::default(),
            watcher: Some(session_watchers()),
        },
    );
    let comparison = session.resolve(&EntryRequest::Working).await.unwrap();
    session.start(comparison).await.unwrap();
    let mut events = session.subscribe();

    std::fs::write(fixture.path().join("new.txt"), "new\n").unwrap();

    let event = tokio::time::timeout(PATIENCE, events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, SessionEvent::Snapshot(version) if version.get() == 2),
        "{event:?}"
    );
    let (_, snapshot) = (0, session.follow_snapshots().borrow().clone().unwrap());
    assert_eq!(snapshot.parts().changed[0].path.as_str(), "new.txt");
    session.close().await;
}
