use super::*;
use crate::domains::changes::change_kind::ChangeKind;
use crate::domains::changes::changed_file::Delta;
use crate::domains::revisions::comment_key::CommentKey;
use crate::domains::revisions::comparison::{Comparison, Liveness};
use crate::domains::revisions::revspec::Revspec;

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

fn file(name: &str, kind: ChangeKind) -> ChangedFile {
    ChangedFile {
        path: path(name),
        kind,
        delta: Delta::Text {
            additions: 1,
            deletions: 0,
        },
        blob: None,
        old_blob: None,
        generated: false,
        submodule: false,
    }
}

/// `kept.txt` unchanged, `new-name.txt` renamed from `old-name.txt`, `gone.txt` deleted, `fresh.txt` added.
fn snapshot() -> Snapshot {
    let working = Comparison::new(
        Revspec::working(),
        Liveness::Worktree,
        CommentKey::working(),
    );
    Snapshot::new(SnapshotParts {
        root: PathBuf::from("/repo"),
        mode: ReviewMode::whole(working),
        version: SnapshotVersion::default().next(),
        old: TreeSource::Object(ObjectId::parse(&"a".repeat(40)).unwrap()),
        new: TreeSource::Worktree,
        head: None,
        context: ContextLines::default(),
        changed: vec![
            file("fresh.txt", ChangeKind::Added),
            file("gone.txt", ChangeKind::Deleted),
            file(
                "new-name.txt",
                ChangeKind::Renamed {
                    from: path("old-name.txt"),
                },
            ),
        ],
        tree: ["fresh.txt", "kept.txt", "new-name.txt"].map(path).into(),
        commit: None,
        commits: SpannedCommits {
            commits: RangeCommits::default(),
            old: TreeSource::Worktree,
            new: TreeSource::Worktree,
        },
    })
}

#[test]
fn the_new_side_exposes_tree_paths_only() {
    let snap = snapshot();
    assert_eq!(
        snap.side_path(&path("kept.txt"), Side::New),
        Some(path("kept.txt"))
    );
    assert_eq!(snap.side_path(&path("gone.txt"), Side::New), None);
    assert_eq!(snap.side_path(&path("old-name.txt"), Side::New), None);
    assert_eq!(snap.side_path(&path(".git/config"), Side::New), None);
    assert_eq!(snap.side_path(&path("ignored.log"), Side::New), None);
}

#[test]
fn the_old_side_maps_a_rename_back_and_reaches_deleted_files() {
    let snap = snapshot();
    assert_eq!(
        snap.side_path(&path("new-name.txt"), Side::Old),
        Some(path("old-name.txt"))
    );
    assert_eq!(
        snap.side_path(&path("gone.txt"), Side::Old),
        Some(path("gone.txt"))
    );
    assert_eq!(
        snap.side_path(&path("old-name.txt"), Side::Old),
        Some(path("old-name.txt"))
    );
    assert_eq!(
        snap.side_path(&path("kept.txt"), Side::Old),
        Some(path("kept.txt"))
    );
    assert_eq!(snap.side_path(&path("ignored.log"), Side::Old), None);
}

#[test]
fn a_file_of_the_review_is_changed_or_in_the_tree_but_never_a_rename_source() {
    let snap = snapshot();
    for present in ["kept.txt", "fresh.txt", "gone.txt", "new-name.txt"] {
        assert!(snap.has_file(&path(present)), "{present}");
    }
    assert!(!snap.has_file(&path("old-name.txt")));
    assert_eq!(
        snap.changed_file(&path("gone.txt")).map(|f| f.kind.code()),
        Some('D')
    );
    assert_eq!(snap.changed_file(&path("kept.txt")), None);
}

#[test]
fn each_side_reads_from_its_source() {
    let snap = snapshot();
    assert_eq!(snap.source(Side::New), &TreeSource::Worktree);
    assert!(snap.source(Side::Old).object().is_some());
    assert_eq!(snap.version().get(), 1);
}
