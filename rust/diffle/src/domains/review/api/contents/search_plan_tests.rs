use std::sync::Arc;

use super::*;
use crate::domains::review::api::api_fixtures::Fixture;

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

async fn snapshot(fixture: &Fixture) -> Arc<Snapshot> {
    let (_, snapshot) = fixture.state.current().await.unwrap();
    snapshot
}

#[tokio::test]
async fn the_diff_scope_is_the_changed_files_and_the_repo_scope_is_everything() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let snapshot = snapshot(&fixture).await;
    assert_eq!(
        files_in_scope(&snapshot, SearchScope::Diff, None),
        Some(vec![path("b.txt")])
    );
    assert_eq!(files_in_scope(&snapshot, SearchScope::Repo, None), None);
}

#[tokio::test]
async fn the_file_scope_is_the_named_file_if_the_review_has_it() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let snapshot = snapshot(&fixture).await;
    let scope = |name: Option<&str>| {
        let path = name.map(path);
        files_in_scope(&snapshot, SearchScope::File, path.as_ref())
    };
    assert_eq!(scope(Some("a.txt")), Some(vec![path("a.txt")]));
    assert_eq!(scope(Some("nope.txt")), Some(Vec::new()));
    assert_eq!(scope(None), Some(Vec::new()));
}

#[tokio::test]
async fn shown_lines_are_the_hunks_of_the_changed_text_files() {
    let fixture = Fixture::opened(&["HEAD^!"]).await;
    let snapshot = snapshot(&fixture).await;
    let active = fixture.session.active().unwrap();
    let (files, ranges) = shown_lines(active.snapshotter(), &snapshot, None)
        .await
        .unwrap();
    assert_eq!(files, vec![path("b.txt")]);
    assert_eq!(ranges[&path("b.txt")], vec![LineSpan::new(1, 1).unwrap()]);

    let (none, _) = shown_lines(active.snapshotter(), &snapshot, Some(&[path("a.txt")]))
        .await
        .unwrap();
    assert!(none.is_empty());
}
