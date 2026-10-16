//! What the session tests share: a watcher driven by hand, and small repositories.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::session::review_session::{Session, SessionConfig};
use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::session::session_watch::{Watch, WatchRequest, WatcherFactory};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

pub(super) const PATIENCE: Duration = Duration::from_secs(10);

/// A watcher the test signals by hand, and sees stopped.
#[derive(Default)]
pub(super) struct FakeWatchers {
    pub(super) started: std::sync::Mutex<Vec<(WatchRequest, mpsc::Sender<()>)>>,
    pub(super) stopped: std::sync::Arc<AtomicBool>,
}

pub(super) struct StopFlag(std::sync::Arc<AtomicBool>);

impl Drop for StopFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl FakeWatchers {
    pub(super) fn factory(self: &Arc<Self>) -> WatcherFactory {
        let fake = Arc::clone(self);
        Arc::new(move |request| {
            let fake = Arc::clone(&fake);
            Box::pin(async move {
                let (signal, dirty) = mpsc::channel(1);
                fake.started.lock().unwrap().push((request, signal));
                Ok(Watch {
                    dirty,
                    stop: Box::new(StopFlag(Arc::clone(&fake.stopped))),
                })
            })
        })
    }

    pub(super) fn signal(&self) -> mpsc::Sender<()> {
        self.started.lock().unwrap().last().unwrap().1.clone()
    }

    pub(super) fn request(&self) -> WatchRequest {
        self.started.lock().unwrap().last().unwrap().0.clone()
    }
}

pub(super) async fn session(
    fixture: &TestRepo,
    follow_refs: FollowRefs,
    watcher: Option<WatcherFactory>,
) -> Arc<Session> {
    let repo = Arc::new(GitRepo::open(fixture.path()).await.unwrap());
    Session::new(
        repo,
        SessionConfig {
            context: ContextLines::default(),
            follow_refs,
            watcher,
        },
    )
}

pub(super) fn revspec(args: &[&str]) -> EntryRequest {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    EntryRequest::Revspec(Revspec::parse(&owned).unwrap())
}

pub(super) async fn open(session: &Arc<Session>, request: &EntryRequest) -> Arc<Snapshot> {
    let comparison = session.resolve(request).await.unwrap();
    session.start(comparison).await.unwrap()
}

pub(super) async fn next_event(
    events: &mut tokio::sync::broadcast::Receiver<SessionEvent>,
) -> SessionEvent {
    tokio::time::timeout(PATIENCE, events.recv())
        .await
        .unwrap()
        .unwrap()
}

pub(super) fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

pub(super) fn write(fixture: &TestRepo, file: &str, content: &str) {
    let target = fixture.path().join(file);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(target, content).unwrap();
}

/// `main` with one commit, then `feat` with two more.
pub(super) fn branched() -> (TestRepo, [ObjectId; 2]) {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.git(&["switch", "--quiet", "--create", "feat"]);
    let first = fixture.commit("b.txt", "b\n");
    let second = fixture.commit("c.txt", "c\n");
    (fixture, [first, second])
}
