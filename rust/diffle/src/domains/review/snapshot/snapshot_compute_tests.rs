use super::*;
use crate::domains::repository::test_repo::TestRepo;
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::resolution::resolve_review::resolve_review;
use crate::domains::revisions::revspec::Revspec;

async fn snapshot_of(fixture: &TestRepo, request: &EntryRequest) -> Snapshot {
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let comparison = resolve_review(&repo, request).await.unwrap();
    let mode = ReviewMode::whole(comparison);
    compute_snapshot(
        &repo,
        &GeneratedDetector::default(),
        &mode,
        SnapshotVersion::default().next(),
        ContextLines::default(),
    )
    .await
    .unwrap()
}

fn revspec(args: &[&str]) -> EntryRequest {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    EntryRequest::Revspec(Revspec::parse(&owned).unwrap())
}

fn tree_of(snapshot: &Snapshot) -> Vec<&str> {
    snapshot
        .parts()
        .tree
        .iter()
        .map(|path| path.as_str())
        .collect()
}

#[tokio::test]
async fn the_working_review_lists_edits_and_untracked_files() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit("b.txt", "b\n");
    std::fs::write(fixture.path().join("a.txt"), "changed\n").unwrap();
    std::fs::write(fixture.path().join("new.txt"), "n\n").unwrap();
    let snapshot = snapshot_of(&fixture, &EntryRequest::Working).await;
    let parts = snapshot.parts();
    let changed: Vec<_> = parts
        .changed
        .iter()
        .map(|f| (f.path.as_str(), f.kind.code()))
        .collect();
    assert_eq!(changed, [("a.txt", 'M'), ("new.txt", 'A')]);
    assert_eq!(tree_of(&snapshot), ["a.txt", "b.txt", "new.txt"]);
    assert_eq!(parts.new, TreeSource::Worktree);
    assert!(parts.head.is_some());
    assert!(parts.commit.is_none());
    // The worktree stands for HEAD, so there is no span to list.
    assert_eq!(parts.commits.commits, RangeCommits::default());
}

#[tokio::test]
async fn a_range_lists_its_commits_oldest_first() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.git(&["switch", "--quiet", "--create", "feat"]);
    let first = fixture.commit("b.txt", "b\n");
    let second = fixture.commit("c.txt", "c\n");
    let snapshot = snapshot_of(&fixture, &revspec(&["main...feat"])).await;
    let spanned = &snapshot.parts().commits;
    let ids: Vec<_> = spanned
        .commits
        .list
        .iter()
        .map(|c| c.info.id.clone())
        .collect();
    assert_eq!(ids, [first, second]);
    assert_eq!(spanned.commits.total, 2);
    assert_eq!(tree_of(&snapshot), ["a.txt", "b.txt", "c.txt"]);
}

#[tokio::test]
async fn a_single_commit_carries_its_message_and_neighbours() {
    let fixture = TestRepo::init();
    let root = fixture.commit("a.txt", "a\n");
    let tip = fixture.commit("b.txt", "b\n");
    let snapshot = snapshot_of(&fixture, &revspec(&["HEAD^!"])).await;
    let commit = snapshot.parts().commit.as_ref().unwrap();
    assert_eq!(commit.info.id, tip);
    assert_eq!(commit.neighbours.parent, Some(root));
    assert_eq!(commit.neighbours.child, None);
    assert_eq!(snapshot.parts().commits.commits.total, 1);
    assert_eq!(snapshot.parts().changed.len(), 1);
}

#[tokio::test]
async fn an_unborn_branch_reviews_its_files_against_the_empty_tree() {
    let fixture = TestRepo::init();
    std::fs::write(fixture.path().join("first.txt"), "1\n").unwrap();
    let snapshot = snapshot_of(&fixture, &EntryRequest::Working).await;
    assert!(snapshot.parts().head.is_none());
    assert_eq!(snapshot.parts().changed.len(), 1);
    assert_eq!(snapshot.parts().commits.commits, RangeCommits::default());
}

#[tokio::test]
async fn a_focused_commit_keeps_listing_its_ranges_commits() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.git(&["switch", "--quiet", "--create", "feat"]);
    let first = fixture.commit("b.txt", "b\n");
    fixture.commit("c.txt", "c\n");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let range = resolve_review(&repo, &revspec(&["main...feat"]))
        .await
        .unwrap();
    let mode = ReviewMode::commit_within(&first, range);
    let snapshot = compute_snapshot(
        &repo,
        &GeneratedDetector::default(),
        &mode,
        SnapshotVersion::default().next(),
        ContextLines::default(),
    )
    .await
    .unwrap();
    let parts = snapshot.parts();
    assert_eq!(parts.changed.len(), 1);
    assert_eq!(
        parts.commit.as_ref().map(|c| c.info.id.clone()),
        Some(first)
    );
    assert_eq!(
        parts.commits.commits.total, 2,
        "the range's commits, not the focused one's"
    );
}
