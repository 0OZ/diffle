use super::*;
use crate::domains::repository::test_repo::TestRepo;

fn file(text: &str) -> WorktreeEntry {
    WorktreeEntry::File(RepoPath::parse(text).unwrap())
}

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

fn id(fixture: &TestRepo, args: &[&str]) -> Option<ObjectId> {
    Some(ObjectId::parse(&fixture.git(args)).unwrap())
}

#[tokio::test]
async fn hashes_files_like_git_does_and_leaves_the_unhashable_without_an_id() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "alpha\n");
    std::fs::write(fixture.path().join("b.txt"), "beta\n").unwrap();
    std::fs::create_dir(fixture.path().join("dir")).unwrap();
    let repo = open(&fixture).await;
    let hashes = repo
        .hash_worktree(&[file("a.txt"), file("b.txt"), file("gone.txt"), file("dir")])
        .await
        .unwrap();
    assert_eq!(hashes[0], id(&fixture, &["rev-parse", "HEAD:a.txt"]));
    assert_eq!(hashes[1], id(&fixture, &["hash-object", "b.txt"]));
    assert_eq!(&hashes[2..], [None, None]);
}

#[tokio::test]
async fn a_path_with_a_newline_is_hashed_on_its_own() {
    let fixture = TestRepo::init();
    std::fs::write(fixture.path().join("a\nb.txt"), "odd\n").unwrap();
    std::fs::write(fixture.path().join("c.txt"), "plain\n").unwrap();
    let repo = open(&fixture).await;
    let hashes = repo
        .hash_worktree(&[file("a\nb.txt"), file("c.txt")])
        .await
        .unwrap();
    assert_eq!(hashes[0], id(&fixture, &["hash-object", "--", "a\nb.txt"]));
    assert_eq!(hashes[1], id(&fixture, &["hash-object", "c.txt"]));
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_hashes_its_target_text() {
    let fixture = TestRepo::init();
    std::os::unix::fs::symlink("elsewhere", fixture.path().join("link")).unwrap();
    fixture.git(&["add", "link"]);
    let repo = open(&fixture).await;
    let hashes = repo.hash_worktree(&[file("link")]).await.unwrap();
    assert_eq!(hashes[0], id(&fixture, &["rev-parse", ":link"]));
}

#[tokio::test]
async fn a_submodule_is_the_commit_it_is_at_and_a_plain_directory_has_none() {
    let fixture = TestRepo::init();
    let inner = TestRepo::init();
    let tip = inner.commit("i.txt", "i\n");
    std::fs::create_dir(fixture.path().join("sub")).unwrap();
    inner.git(&[
        "clone",
        "--quiet",
        ".",
        fixture.path().join("sub").to_str().unwrap(),
    ]);
    std::fs::create_dir(fixture.path().join("plain")).unwrap();
    let repo = open(&fixture).await;
    let hashes = repo
        .hash_worktree(&[
            WorktreeEntry::Submodule(RepoPath::parse("sub").unwrap()),
            WorktreeEntry::Submodule(RepoPath::parse("plain").unwrap()),
        ])
        .await
        .unwrap();
    assert_eq!(hashes, [Some(tip), None]);
}

#[tokio::test]
async fn hashing_content_applies_the_clean_filters_of_the_path() {
    let fixture = TestRepo::init();
    let repo = open(&fixture).await;
    let path = RepoPath::parse("a.txt").unwrap();
    let hashed = repo.hash_content(b"alpha\n", &path).await.unwrap();
    std::fs::write(fixture.path().join("a.txt"), "alpha\n").unwrap();
    assert_eq!(hashed, id(&fixture, &["hash-object", "a.txt"]));
}
