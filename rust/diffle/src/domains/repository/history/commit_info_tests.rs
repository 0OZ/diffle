use super::*;
use crate::domains::repository::test_repo::TestRepo;

#[tokio::test]
async fn reads_the_hash_abbreviation_and_whole_message() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "subject",
        "-m",
        "body line",
    ]);
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    let info = repo.commit_info(&RevName::head()).await.unwrap().unwrap();
    assert_eq!(info.message, "subject\n\nbody line");
    assert!(info.id.as_str().starts_with(&info.short));
    assert_eq!(info.id.as_str(), fixture.git(&["rev-parse", "HEAD"]));
}

#[tokio::test]
async fn a_name_that_is_no_commit_is_absent() {
    let fixture = TestRepo::init();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.commit_info(&RevName::head()).await.unwrap(), None);
    fixture.commit("a.txt", "a\n");
    assert_eq!(repo.commit_behind_head(1).await.unwrap(), None);
}

#[tokio::test]
async fn counts_back_from_head() {
    let fixture = TestRepo::init();
    let first = fixture.commit("a.txt", "1\n");
    let second = fixture.commit("a.txt", "2\n");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(
        repo.commit_behind_head(0).await.unwrap().unwrap().id,
        second
    );
    assert_eq!(repo.commit_behind_head(1).await.unwrap().unwrap().id, first);
}
