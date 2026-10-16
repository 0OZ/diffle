use super::*;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

#[tokio::test]
async fn lists_the_index_a_tree_and_the_untracked_files() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("dir/a.txt", "a\n");
    fixture.commit("b.txt", "b\n");
    std::fs::write(fixture.path().join("new.txt"), "n\n").unwrap();
    std::fs::write(fixture.path().join(".gitignore"), "skip.log\n").unwrap();
    std::fs::write(fixture.path().join("skip.log"), "x\n").unwrap();
    let repo = open(&fixture).await;
    assert_eq!(repo.index_paths().await.unwrap(), ["b.txt", "dir/a.txt"]);
    assert_eq!(repo.tree_paths(&commit).await.unwrap(), ["dir/a.txt"]);
    let untracked = repo.untracked_paths().await.unwrap();
    assert_eq!(untracked, [".gitignore", "new.txt"]);
    assert_eq!(repo.ignored_entries().await.unwrap(), ["skip.log"]);
}

#[tokio::test]
async fn tracked_means_in_the_index_and_a_name_is_not_a_pattern() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    fixture.commit("[x].txt", "x\n");
    std::fs::write(fixture.path().join("untracked.txt"), "u\n").unwrap();
    let repo = open(&fixture).await;
    assert!(repo.is_tracked(&path("a.txt")).await.unwrap());
    assert!(!repo.is_tracked(&path("untracked.txt")).await.unwrap());
    assert!(repo.is_tracked(&path("[x].txt")).await.unwrap());
    // Read as a pattern, `*.txt` would match tracked files.
    assert!(!repo.is_tracked(&path("*.txt")).await.unwrap());
}

#[tokio::test]
async fn reports_the_paths_git_ignores() {
    let fixture = TestRepo::init();
    std::fs::write(fixture.path().join(".gitignore"), "*.log\n").unwrap();
    let repo = open(&fixture).await;
    let paths = ["a.log".to_owned(), "b.txt".to_owned()];
    let ignored = repo.ignored_among(&paths).await.unwrap();
    assert_eq!(ignored, HashSet::from(["a.log".to_owned()]));
    assert!(repo.ignored_among(&[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn attributes_read_from_the_checkout_or_from_a_commit() {
    let fixture = TestRepo::init();
    fixture.commit(
        ".gitattributes",
        "gen.txt linguist-generated\nkeep.txt -linguist-generated\n",
    );
    let commit = fixture.commit("gen.txt", "g\n");
    let repo = open(&fixture).await;
    let (gen_path, keep, other) = (path("gen.txt"), path("keep.txt"), path("other.txt"));
    let paths = [&gen_path, &keep, &other];
    std::fs::write(
        fixture.path().join(".gitattributes"),
        "other.txt linguist-generated\n",
    )
    .unwrap();
    let at_commit = repo
        .bool_attribute("linguist-generated", &paths, &TreeSource::Object(commit))
        .await
        .unwrap();
    assert_eq!(at_commit.get("gen.txt"), Some(&AttrValue::Set));
    assert_eq!(at_commit.get("keep.txt"), Some(&AttrValue::Unset));
    assert_eq!(at_commit.get("other.txt"), None);
    let on_disk = repo
        .bool_attribute("linguist-generated", &paths, &TreeSource::Worktree)
        .await
        .unwrap();
    assert_eq!(on_disk.get("other.txt"), Some(&AttrValue::Set));
    assert_eq!(on_disk.get("gen.txt"), None);
}
