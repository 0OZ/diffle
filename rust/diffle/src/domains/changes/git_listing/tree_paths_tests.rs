use super::*;
use crate::domains::changes::changed_files::changed_files;
use crate::domains::repository::test_repo::TestRepo;

fn names(paths: &BTreeSet<RepoPath>) -> Vec<&str> {
    paths.iter().map(RepoPath::as_str).collect()
}

#[tokio::test]
async fn a_commit_lists_its_own_tree() {
    let fixture = TestRepo::init();
    fixture.commit("b.txt", "b\n");
    let tip = fixture.commit("a/c.txt", "c\n");
    std::fs::write(fixture.path().join("later.txt"), "x\n").unwrap();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let paths = listed_new_side(&repo, &TreeSource::Object(tip))
        .await
        .unwrap();
    assert_eq!(names(&paths), ["a/c.txt", "b.txt"]);
}

#[tokio::test]
async fn the_worktree_is_the_index_and_the_untracked_with_the_changes_applied() {
    let fixture = TestRepo::init();
    fixture.commit("keep.txt", "k\n");
    fixture.commit("old.txt", "same content here\nmore\n");
    let base = fixture.commit("gone.txt", "g\n");
    std::fs::remove_file(fixture.path().join("gone.txt")).unwrap();
    fixture.git(&["mv", "old.txt", "moved.txt"]);
    std::fs::write(fixture.path().join("new.txt"), "n\n").unwrap();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let old = TreeSource::Object(base);
    let changed = changed_files(&repo, &old, &TreeSource::Worktree)
        .await
        .unwrap();
    let listed = listed_new_side(&repo, &TreeSource::Worktree).await.unwrap();
    let paths = with_changes(listed, &changed);
    assert_eq!(names(&paths), ["keep.txt", "moved.txt", "new.txt"]);
}
