use super::*;
use crate::domains::repository::test_repo::TestRepo;

#[tokio::test]
async fn a_commit_in_the_middle_has_a_parent_and_a_child() {
    let fixture = TestRepo::init();
    let first = fixture.commit("a.txt", "1\n");
    let second = fixture.commit("a.txt", "2\n");
    let third = fixture.commit("a.txt", "3\n");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let middle = repo.commit_neighbours(&second).await.unwrap();
    assert_eq!(
        (middle.parent, middle.child),
        (Some(first.clone()), Some(third.clone()))
    );
    let root = repo.commit_neighbours(&first).await.unwrap();
    assert_eq!((root.parent, root.child), (None, Some(second)));
    let head = repo.commit_neighbours(&third).await.unwrap();
    assert_eq!(head.child, None);
}

#[tokio::test]
async fn a_merged_side_branch_has_no_child_on_the_first_parent_line() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "1\n");
    fixture.git(&["switch", "--quiet", "--create", "side"]);
    let side = fixture.commit("b.txt", "side\n");
    fixture.git(&["switch", "--quiet", "main"]);
    fixture.commit("c.txt", "main\n");
    fixture.git(&["merge", "--quiet", "--no-ff", "-m", "merge", "side"]);
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.commit_neighbours(&side).await.unwrap().child, None);
}

#[tokio::test]
async fn an_unborn_head_leaves_no_child() {
    let fixture = TestRepo::init();
    let lone = fixture.commit("a.txt", "1\n");
    fixture.git(&["switch", "--quiet", "--orphan", "fresh"]);
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.commit_neighbours(&lone).await.unwrap().child, None);
}
