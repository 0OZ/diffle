use super::*;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

#[tokio::test]
async fn lists_a_span_oldest_first_with_author_and_total() {
    let fixture = TestRepo::init();
    let base = fixture.commit("a.txt", "0\n");
    let one = fixture.commit("a.txt", "1\n");
    let two = fixture.commit("a.txt", "2\n");
    let repo = open(&fixture).await;
    let range = CommitRange::Span {
        from: base,
        to: two.clone(),
    };
    let commits = repo.range_commits(&range, 10).await.unwrap();
    assert_eq!(commits.total, 2);
    let ids: Vec<_> = commits.list.iter().map(|c| c.info.id.clone()).collect();
    assert_eq!(ids, [one, two]);
    let first = &commits.list[0];
    assert_eq!(
        (first.author.as_str(), first.email.as_str()),
        ("Test", "test@example.com")
    );
    assert_eq!(first.info.message, "edit a.txt");
    assert!(first.date_ms > 1_600_000_000_000);
}

#[tokio::test]
async fn keeps_the_newest_when_the_range_is_longer_than_the_limit() {
    let fixture = TestRepo::init();
    let base = fixture.commit("a.txt", "0\n");
    fixture.commit("a.txt", "1\n");
    let second = fixture.commit("a.txt", "2\n");
    let third = fixture.commit("a.txt", "3\n");
    let repo = open(&fixture).await;
    let range = CommitRange::Span {
        from: base,
        to: third.clone(),
    };
    let commits = repo.range_commits(&range, 2).await.unwrap();
    assert_eq!(commits.total, 3);
    let ids: Vec<_> = commits.list.iter().map(|c| c.info.id.clone()).collect();
    assert_eq!(ids, [second, third]);
}

#[tokio::test]
async fn one_commit_alone_and_multiline_messages() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "0\n");
    fixture.git(&[
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "subject",
        "-m",
        "second\nparagraph",
    ]);
    let tip = ObjectId::parse(&fixture.git(&["rev-parse", "HEAD"])).unwrap();
    let repo = open(&fixture).await;
    let commits = repo
        .range_commits(&CommitRange::Single(tip), 5)
        .await
        .unwrap();
    assert_eq!(commits.total, 1);
    assert_eq!(commits.list[0].info.message, "subject\n\nsecond\nparagraph");
}

#[tokio::test]
async fn an_empty_range_has_no_commits() {
    let fixture = TestRepo::init();
    let tip = fixture.commit("a.txt", "0\n");
    let repo = open(&fixture).await;
    let range = CommitRange::Span {
        from: tip.clone(),
        to: tip,
    };
    assert_eq!(
        repo.range_commits(&range, 5).await.unwrap(),
        RangeCommits::default()
    );
}
