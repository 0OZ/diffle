use std::sync::Arc;

use std::time::{Duration, Instant};

use super::*;
use crate::domains::changes::change_kind::ChangeKind;
use crate::domains::changes::changed_file::Delta;
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

fn changed(name: &str, kind: ChangeKind, delta: Delta) -> ChangedFile {
    ChangedFile {
        path: path(name),
        kind,
        delta,
        blob: None,
        old_blob: None,
        generated: false,
        submodule: false,
    }
}

fn text() -> Delta {
    Delta::Text {
        additions: 1,
        deletions: 0,
    }
}

#[test]
fn only_changed_text_files_that_still_exist_are_searchable() {
    let files = [
        changed("edited.txt", ChangeKind::Modified, text()),
        changed("added.txt", ChangeKind::Added, text()),
        changed("gone.txt", ChangeKind::Deleted, text()),
        changed("image.png", ChangeKind::Modified, Delta::Binary),
    ];
    assert_eq!(
        searchable_files(&files, None),
        [path("edited.txt"), path("added.txt")]
    );
    let some = [path("added.txt"), path("gone.txt"), path("image.png")];
    assert_eq!(searchable_files(&files, Some(&some)), [path("added.txt")]);
    assert!(searchable_files(&files, Some(&[])).is_empty());
}

/// A change set of this size made the first search spin for minutes when each file was
/// looked up by scanning the list.
#[test]
fn choosing_among_a_huge_change_set_takes_linear_time() {
    const FILES: usize = 100_000;
    let files: Vec<ChangedFile> = (0..FILES)
        .map(|n| {
            changed(
                &format!("pkg/file-{n:06}.txt"),
                ChangeKind::Modified,
                text(),
            )
        })
        .collect();
    // Reversed, so a scan that stops at the first hit still walks about half the list.
    let within: Vec<RepoPath> = files.iter().rev().map(|file| file.path.clone()).collect();

    let started = Instant::now();
    let chosen = searchable_files(&files, Some(&within));

    assert_eq!(chosen.len(), FILES);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "took {:?}",
        started.elapsed()
    );
}
