use super::*;
use crate::domains::changes::change_kind::ChangeKind;
use crate::domains::repository::names::repo_path::RepoPath;

fn file(path: &str, delta: Delta) -> ChangedFile {
    ChangedFile {
        path: RepoPath::parse(path).unwrap(),
        kind: ChangeKind::Modified,
        delta,
        blob: None,
        old_blob: None,
        generated: false,
        submodule: false,
    }
}

fn text(additions: u32, deletions: u32) -> Delta {
    Delta::Text {
        additions,
        deletions,
    }
}

#[test]
fn the_totals_count_lines_and_skip_binary_files() {
    let changed = [
        file("a.rs", text(3, 1)),
        file("b.rs", text(2, 0)),
        file("c.png", Delta::Binary),
    ];
    assert_eq!(
        report_lines("main...HEAD", &changed, Liveness::Pinned, false, true),
        ["🔍 comparing main...HEAD", "📝 3 changed files  +5 −1"]
    );
}

#[test]
fn one_file_is_singular() {
    let lines = report_lines(
        "a^!",
        &[file("a.rs", text(1, 1))],
        Liveness::Pinned,
        false,
        true,
    );
    assert_eq!(lines[1], "📝 1 changed file  +1 −1");
}

#[test]
fn no_differences_say_what_to_do_next() {
    let lines = report_lines("HEAD..worktree", &[], Liveness::Pinned, false, true);
    assert_eq!(
        lines[1],
        "! No differences. Open any file from the tree to comment on it."
    );
}

#[test]
fn a_live_review_says_what_it_watches() {
    let changed = [file("a.rs", text(1, 0))];
    let watching = |liveness, follows, on| {
        report_lines("x", &changed, liveness, follows, on)
            .pop()
            .unwrap()
    };
    assert_eq!(
        watching(Liveness::Worktree, true, true),
        "👀 watching the worktree"
    );
    assert_eq!(watching(Liveness::Refs, true, true), "👀 watching refs");
    assert_eq!(
        watching(Liveness::Refs, false, true),
        "👀 watching refs; moved refs show a Reload button"
    );
    assert_eq!(
        watching(Liveness::Worktree, true, false),
        "👀 watching the worktree (disabled with --no-watch)"
    );
}

#[test]
fn a_pinned_review_watches_nothing() {
    let lines = report_lines(
        "a^!",
        &[file("a.rs", text(1, 0))],
        Liveness::Pinned,
        false,
        true,
    );
    assert_eq!(lines.len(), 2);
}
