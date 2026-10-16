use std::sync::Arc;

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::review::session::review_session::{Session, SessionConfig};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;
use crate::kernel::side::Side;

const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\nfirst";
const REPAINTED: &[u8] = b"\x89PNG\r\n\x1a\nsecond";

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

async fn opened(fixture: &TestRepo, args: &[&str]) -> (Arc<Session>, Arc<Snapshot>) {
    let repo = Arc::new(GitRepo::open(fixture.path()).await.unwrap());
    let session = Session::new(
        repo,
        SessionConfig {
            context: ContextLines::default(),
            follow_refs: FollowRefs::Auto,
            watcher: None,
        },
    );
    let request = match args {
        [] => EntryRequest::Working,
        args => {
            let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
            EntryRequest::Revspec(Revspec::parse(&owned).unwrap())
        }
    };
    let comparison = session.resolve(&request).await.unwrap();
    let snapshot = session.start(comparison).await.unwrap();
    (session, snapshot)
}

fn commit_bytes(fixture: &TestRepo, name: &str, bytes: &[u8]) {
    std::fs::write(fixture.path().join(name), bytes).unwrap();
    fixture.git(&["add", name]);
    fixture.git(&["commit", "--quiet", "-m", name]);
}

#[tokio::test]
async fn a_changed_file_is_named_by_its_blob_on_each_side() {
    let fixture = TestRepo::init();
    commit_bytes(&fixture, "logo.png", PICTURE);
    commit_bytes(&fixture, "logo.png", REPAINTED);
    let (_, snapshot) = opened(&fixture, &["HEAD^!"]).await;
    let file = snapshot.changed_file(&path("logo.png")).unwrap();
    assert_eq!(
        snapshot.image_key(&path("logo.png"), Side::New).as_deref(),
        file.blob.as_ref().map(|blob| blob.as_str())
    );
    assert_eq!(
        snapshot.image_key(&path("logo.png"), Side::Old).as_deref(),
        file.old_blob.as_ref().map(|blob| blob.as_str())
    );
}

#[tokio::test]
async fn an_unchanged_path_is_named_by_the_commit_of_a_side() {
    let fixture = TestRepo::init();
    commit_bytes(&fixture, "logo.png", PICTURE);
    commit_bytes(&fixture, "other.txt", b"x");
    let (_, snapshot) = opened(&fixture, &["HEAD^!"]).await;
    let new = snapshot.parts().new.object().unwrap().as_str().to_owned();
    assert_eq!(snapshot.image_key(&path("logo.png"), Side::New), Some(new));
}

#[tokio::test]
async fn a_key_reads_its_own_bytes_and_nothing_else() {
    let fixture = TestRepo::init();
    commit_bytes(&fixture, "logo.png", PICTURE);
    commit_bytes(&fixture, "logo.png", REPAINTED);
    let (session, snapshot) = opened(&fixture, &["HEAD^!"]).await;
    let logo = path("logo.png");
    let new_key = snapshot.image_key(&logo, Side::New).unwrap();
    let old_key = snapshot.image_key(&logo, Side::Old).unwrap();

    let new = session
        .read_image(&snapshot, &logo, Side::New, &new_key)
        .await;
    assert_eq!(new.unwrap().as_deref(), Some(REPAINTED));
    let old = session
        .read_image(&snapshot, &logo, Side::Old, &old_key)
        .await;
    assert_eq!(old.unwrap().as_deref(), Some(PICTURE));
    // The other side's key names other bytes.
    let crossed = session
        .read_image(&snapshot, &logo, Side::New, &old_key)
        .await;
    assert_eq!(crossed.unwrap(), None);
    let guessed = session
        .read_image(&snapshot, &logo, Side::New, "nope")
        .await;
    assert_eq!(guessed.unwrap(), None);
}

#[tokio::test]
async fn a_path_outside_the_snapshot_reads_as_absent() {
    let fixture = TestRepo::init();
    commit_bytes(&fixture, "logo.png", PICTURE);
    commit_bytes(&fixture, "other.txt", b"x");
    let (session, snapshot) = opened(&fixture, &["HEAD^!"]).await;
    let stranger = path("secret.png");
    let key = snapshot.parts().new.object().unwrap().as_str().to_owned();
    let read = session
        .read_image(&snapshot, &stranger, Side::New, &key)
        .await;
    assert_eq!(read.unwrap(), None);
}

#[tokio::test]
async fn a_worktree_picture_is_served_only_while_it_hashes_to_its_key() {
    let fixture = TestRepo::init();
    commit_bytes(&fixture, "other.txt", b"x");
    std::fs::write(fixture.path().join("logo.png"), PICTURE).unwrap();
    let (session, snapshot) = opened(&fixture, &[]).await;
    let logo = path("logo.png");
    let key = snapshot.image_key(&logo, Side::New).unwrap();
    let served = session.read_image(&snapshot, &logo, Side::New, &key).await;
    assert_eq!(served.unwrap().as_deref(), Some(PICTURE));

    std::fs::write(fixture.path().join("logo.png"), REPAINTED).unwrap();
    let stale = session.read_image(&snapshot, &logo, Side::New, &key).await;
    assert_eq!(stale.unwrap(), None);
}
