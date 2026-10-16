use super::*;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::revisions::comment_key::CommentKey;
use crate::domains::revisions::comparison::{Comparison, Liveness};
use crate::domains::revisions::revspec::Revspec;

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

async fn working_snapshotter(fixture: &TestRepo) -> Snapshotter {
    let repo = Arc::new(GitRepo::open(fixture.path()).await.unwrap());
    let comparison = Comparison::new(
        Revspec::working(),
        Liveness::Worktree,
        CommentKey::working(),
    );
    Snapshotter::new(
        repo,
        ReviewMode::whole(comparison),
        SnapshotVersion::default().next(),
        ContextLines::default(),
    )
}

#[tokio::test]
async fn caches_until_invalidated_and_carries_the_new_version() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let snapshotter = working_snapshotter(&fixture).await;
    let first = snapshotter.current().await.unwrap();
    assert!(first.parts().changed.is_empty());
    std::fs::write(fixture.path().join("a.txt"), "edited\n").unwrap();
    assert!(Arc::ptr_eq(&first, &snapshotter.current().await.unwrap()));

    let second_version = first.version().next();
    snapshotter.invalidate(second_version);
    let second = snapshotter.current().await.unwrap();
    assert_eq!(second.version(), second_version);
    assert_eq!(second.parts().changed.len(), 1);
}

#[tokio::test]
async fn patches_follow_the_snapshot_and_unknown_paths_have_none() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit("b.txt", "b\n");
    std::fs::write(fixture.path().join("a.txt"), "a2\n").unwrap();
    std::fs::write(fixture.path().join("b.txt"), "b2\n").unwrap();
    let snapshotter = working_snapshotter(&fixture).await;
    let patch = snapshotter.patch(&path("a.txt")).await.unwrap().unwrap();
    assert!(patch.contains("+a2"), "{patch}");
    assert!(snapshotter.patch(&path("c.txt")).await.unwrap().is_none());

    let all = snapshotter.patch_all().await.unwrap();
    assert!(all.contains("+a2") && all.contains("+b2"));
    let many = snapshotter
        .patch_many(&[
            path("b.txt"),
            path("missing.txt"),
            path("b.txt"),
            path("a.txt"),
        ])
        .await
        .unwrap();
    assert_eq!(many.matches("+b2").count(), 1);
    assert!(many.contains("+a2"));
}

#[tokio::test]
async fn a_new_context_drops_the_patches_and_shows_in_the_snapshot() {
    let fixture = TestRepo::init();
    let lines: String = (1..=20).map(|n| format!("line {n}\n")).collect();
    fixture.commit("a.txt", &lines);
    std::fs::write(
        fixture.path().join("a.txt"),
        lines.replace("line 10\n", "ten\n"),
    )
    .unwrap();
    let snapshotter = working_snapshotter(&fixture).await;
    let narrow = snapshotter.patch(&path("a.txt")).await.unwrap().unwrap();
    let version = snapshotter.current().await.unwrap().version().next();
    snapshotter.set_context(ContextLines::new(1.0).unwrap(), version);
    let tight = snapshotter.patch(&path("a.txt")).await.unwrap().unwrap();
    assert!(tight.len() < narrow.len());
    assert_eq!(
        snapshotter.current().await.unwrap().parts().context.get(),
        1
    );
    // Setting what is already set keeps the generation.
    let same = snapshotter.current().await.unwrap();
    snapshotter.set_context(ContextLines::new(1.0).unwrap(), version.next());
    assert!(Arc::ptr_eq(&same, &snapshotter.current().await.unwrap()));
}

#[tokio::test]
async fn a_failed_computation_is_not_remembered() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    let repo = Arc::new(GitRepo::open(fixture.path()).await.unwrap());
    // `nope` does not resolve, so every snapshot fails until the ref exists.
    let spec = Revspec::parse(&["nope..HEAD".to_owned()]).unwrap();
    let comparison = Comparison::new(spec, Liveness::Refs, CommentKey::working());
    let snapshotter = Snapshotter::new(
        repo,
        ReviewMode::whole(comparison),
        SnapshotVersion::default().next(),
        ContextLines::default(),
    );
    assert!(snapshotter.current().await.is_err());
    fixture.git(&["branch", "nope"]);
    assert!(snapshotter.current().await.is_ok());
}
