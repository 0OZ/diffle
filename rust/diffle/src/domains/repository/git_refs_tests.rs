use super::*;
use crate::domains::repository::test_repo::TestRepo;

fn rev(text: &str) -> RevName {
    RevName::parse(text).unwrap()
}

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

#[test]
fn only_heads_and_remotes_are_branches() {
    assert_eq!(
        BranchRef::parse("refs/heads/feat/x"),
        Some(BranchRef::Local("feat/x".into()))
    );
    assert_eq!(
        BranchRef::parse("refs/remotes/origin/main"),
        Some(BranchRef::Remote("origin/main".into()))
    );
    assert_eq!(BranchRef::parse("refs/tags/v1"), None);
    assert_eq!(BranchRef::parse(""), None);
    assert_eq!(BranchRef::Local("a".into()).full_name(), "refs/heads/a");
}

#[tokio::test]
async fn names_the_branch_of_head_and_of_a_branch_but_not_of_a_hash() {
    let fixture = TestRepo::init();
    let tip = fixture.commit("a.txt", "a");
    let repo = open(&fixture).await;
    let main = Some(BranchRef::Local("main".into()));
    assert_eq!(repo.branch_ref(&RevName::head()).await.unwrap(), main);
    assert_eq!(repo.branch_ref(&rev("main")).await.unwrap(), main);
    assert_eq!(
        repo.branch_ref(&RevName::of_object(&tip)).await.unwrap(),
        None
    );
    assert_eq!(repo.branch_ref(&rev("HEAD~0")).await.unwrap(), None);
    assert_eq!(repo.branch_ref(&rev("nope")).await.unwrap(), None);
}

#[tokio::test]
async fn a_detached_head_names_no_branch() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a");
    fixture.git(&["switch", "--quiet", "--detach"]);
    assert_eq!(
        open(&fixture)
            .await
            .branch_ref(&RevName::head())
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn lists_remotes_once_each_with_the_configured_url() {
    let fixture = TestRepo::init();
    fixture.git(&["remote", "add", "origin", "git@github.com:o/r.git"]);
    fixture.git(&["remote", "add", "a/b", "https://example.com/x"]);
    fixture.git(&[
        "config",
        "--add",
        "remote.origin.url",
        "https://second.example/ignored",
    ]);
    let remotes = open(&fixture).await.remotes().await.unwrap();
    let listed: Vec<(&str, &str)> = remotes
        .iter()
        .map(|remote| (remote.name.as_str(), remote.url.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("origin", "git@github.com:o/r.git"),
            ("a/b", "https://example.com/x")
        ]
    );
}

#[tokio::test]
async fn a_repository_without_remotes_lists_none() {
    let fixture = TestRepo::init();
    assert_eq!(open(&fixture).await.remotes().await.unwrap(), []);
}

#[tokio::test]
async fn a_local_branch_follows_the_remote_branch_it_tracks() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a");
    fixture.git(&["remote", "add", "origin", "git@github.com:o/r.git"]);
    fixture.git(&["config", "branch.main.remote", "origin"]);
    fixture.git(&["config", "branch.main.merge", "refs/heads/trunk"]);
    let repo = open(&fixture).await;
    let upstream = repo
        .upstream_branch(&BranchRef::Local("main".into()), &[])
        .await
        .unwrap();
    assert_eq!(
        upstream,
        Some(Upstream {
            remote: "origin".into(),
            branch: "trunk".into()
        })
    );
}

#[tokio::test]
async fn an_untracked_branch_has_no_upstream() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a");
    let repo = open(&fixture).await;
    assert_eq!(
        repo.upstream_branch(&BranchRef::Local("main".into()), &[])
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_remote_tracking_branch_belongs_to_the_longest_matching_remote() {
    let fixture = TestRepo::init();
    let repo = open(&fixture).await;
    let remotes = [
        Remote {
            name: "a".into(),
            url: String::new(),
        },
        Remote {
            name: "a/b".into(),
            url: String::new(),
        },
    ];
    let upstream = repo
        .upstream_branch(&BranchRef::Remote("a/b/feat".into()), &remotes)
        .await
        .unwrap();
    assert_eq!(
        upstream,
        Some(Upstream {
            remote: "a/b".into(),
            branch: "feat".into()
        })
    );
}
