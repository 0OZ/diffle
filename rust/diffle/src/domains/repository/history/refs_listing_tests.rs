use super::*;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

#[tokio::test]
async fn lists_branches_tags_and_recent_commits() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "1\n");
    fixture.git(&["tag", "v1"]);
    fixture.git(&["switch", "--quiet", "--create", "feat"]);
    let tip = fixture.commit("a.txt", "2\n");
    let repo = open(&fixture).await;
    let listing = repo.refs_listing().await.unwrap();
    assert_eq!(listing.current.as_deref(), Some("feat"));
    assert_eq!(listing.default_branch.as_deref(), Some("main"));
    assert_eq!(listing.branches.len(), 2);
    assert!(
        listing.branches.contains(&"main".to_owned())
            && listing.branches.contains(&"feat".to_owned())
    );
    assert_eq!(listing.tags, ["v1"]);
    assert_eq!(listing.recent.len(), 2);
    assert_eq!(listing.recent[0].sha, tip.as_str());
    assert_eq!(listing.recent[0].subject, "edit a.txt");
}

#[tokio::test]
async fn a_detached_head_has_no_current_branch() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "1\n");
    fixture.git(&["switch", "--quiet", "--detach"]);
    let repo = open(&fixture).await;
    assert_eq!(repo.refs_listing().await.unwrap().current, None);
}

#[tokio::test]
async fn an_unborn_branch_still_lists_what_it_can() {
    let fixture = TestRepo::init();
    let repo = open(&fixture).await;
    let listing = repo.refs_listing().await.unwrap();
    assert_eq!(listing, RefsListing::default());
}

#[tokio::test]
async fn remote_branches_skip_the_symbolic_head_and_origin_head_is_the_default() {
    let remote = TestRepo::init();
    remote.commit("a.txt", "1\n");
    let fixture = TestRepo::init();
    fixture.git(&["remote", "add", "origin", remote.path().to_str().unwrap()]);
    fixture.git(&["fetch", "--quiet", "origin"]);
    fixture.git(&["remote", "set-head", "origin", "main"]);
    let repo = open(&fixture).await;
    let listing = repo.refs_listing().await.unwrap();
    assert_eq!(listing.remote_branches, ["origin/main"]);
    assert_eq!(listing.default_branch.as_deref(), Some("origin/main"));
}
