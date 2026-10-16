use super::*;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

#[test]
fn rewrites_the_header_of_an_added_file() {
    let forward = "diff --git a/dev/null b/src/new.rs\nnew file mode 100644\n";
    assert_eq!(
        conventional_header(forward, false),
        "diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n"
    );
    let quoted = "diff --git a/dev/null \"b/sp\\tace.rs\"\n+x\n";
    assert_eq!(
        conventional_header(quoted, false),
        "diff --git \"a/sp\\tace.rs\" \"b/sp\\tace.rs\"\n+x\n"
    );
}

#[test]
fn rewrites_the_header_of_a_deleted_file_when_reversed() {
    let plain = "diff --git a/old.txt b/dev/null\ndeleted file mode 100644\n";
    assert_eq!(
        conventional_header(plain, true),
        "diff --git a/old.txt b/old.txt\ndeleted file mode 100644\n"
    );
    let quoted = "diff --git \"a/q\\\"uote\" b/dev/null\n";
    assert_eq!(
        conventional_header(quoted, true),
        "diff --git \"a/q\\\"uote\" \"b/q\\\"uote\"\n"
    );
}

#[test]
fn leaves_other_headers_and_later_lines_alone() {
    let text = "diff --git a/x b/x\n--- a/x\n+++ b/x\n";
    assert_eq!(conventional_header(text, false), text);
    assert_eq!(conventional_header(text, true), text);
    assert_eq!(conventional_header("", false), "");
}

#[tokio::test]
async fn patches_one_file_or_all_of_them_between_a_commit_and_the_worktree() {
    let fixture = TestRepo::init();
    let base = fixture.commit("a.txt", "one\n");
    fixture.commit("b.txt", "bee\n");
    std::fs::write(fixture.path().join("a.txt"), "one\ntwo\n").unwrap();
    let repo = open(&fixture).await;
    let sides = DiffSides::new(&TreeSource::Object(base), &TreeSource::Worktree);
    let a = repo.patch_text(&sides, 3, &[&path("a.txt")]).await.unwrap();
    assert!(a.starts_with("diff --git a/a.txt b/a.txt\n"), "{a}");
    assert!(a.contains("+two\n"));
    let all = repo.patch_text(&sides, 3, &[]).await.unwrap();
    assert!(all.contains("b/b.txt") && all.contains("a/a.txt"), "{all}");
}

#[tokio::test]
async fn a_reversed_diff_still_reads_old_to_new() {
    let fixture = TestRepo::init();
    let head = fixture.commit("a.txt", "committed\n");
    std::fs::write(fixture.path().join("a.txt"), "edited\n").unwrap();
    let repo = open(&fixture).await;
    let sides = DiffSides::new(&TreeSource::Worktree, &TreeSource::Object(head));
    let patch = repo.patch_text(&sides, 3, &[]).await.unwrap();
    assert!(patch.contains("-edited\n+committed\n"), "{patch}");
    assert!(patch.starts_with("diff --git a/a.txt b/a.txt\n"), "{patch}");
}

#[tokio::test]
async fn untracked_files_diff_against_nothing_in_both_directions() {
    let fixture = TestRepo::init();
    let head = fixture.commit("a.txt", "a\n");
    std::fs::write(fixture.path().join("new.txt"), "fresh\n").unwrap();
    let repo = open(&fixture).await;
    let forward = DiffSides::new(&TreeSource::Object(head.clone()), &TreeSource::Worktree);
    let added = repo
        .untracked_patch_text(&forward, 3, &path("new.txt"))
        .await
        .unwrap();
    assert!(
        added.starts_with("diff --git a/new.txt b/new.txt\nnew file mode"),
        "{added}"
    );
    assert!(added.contains("+fresh\n"));
    let backward = DiffSides::new(&TreeSource::Worktree, &TreeSource::Object(head));
    let removed = repo
        .untracked_patch_text(&backward, 3, &path("new.txt"))
        .await
        .unwrap();
    assert!(
        removed.starts_with("diff --git a/new.txt b/new.txt\ndeleted file mode"),
        "{removed}"
    );
    assert!(removed.contains("-fresh\n"));
}
