use super::*;
use crate::domains::repository::test_repo::TestRepo;

const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn rev(text: &str) -> RevName {
    RevName::parse(text).unwrap()
}

#[tokio::test]
async fn opening_outside_a_repository_names_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let error = GitRepo::open(dir.path()).await.unwrap_err();
    assert!(matches!(error, GitError::NotARepository { dir: d } if d == dir.path()));
}

#[tokio::test]
async fn opening_a_missing_directory_or_a_file_is_not_a_git_failure() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing");
    let error = GitRepo::open(&missing).await.unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("not a directory: {}", missing.display())
    );

    let file = dir.path().join("file.txt");
    std::fs::write(&file, "x").unwrap();
    let error = GitRepo::open(&file).await.unwrap_err();
    assert!(matches!(error, GitError::NotADirectory { dir: d } if d == file));
}

#[tokio::test]
async fn opening_a_subdirectory_finds_the_root_and_git_dirs() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a");
    std::fs::create_dir(fixture.path().join("sub")).unwrap();
    let repo = GitRepo::open(&fixture.path().join("sub")).await.unwrap();
    assert_eq!(
        repo.root().canonicalize().unwrap(),
        fixture.path().canonicalize().unwrap()
    );
    assert_eq!(repo.git_dir(), repo.root().join(".git"));
    assert_eq!(repo.common_dir(), repo.git_dir());
}

#[tokio::test]
async fn resolves_a_commit_by_name_and_reports_an_unknown_one_as_absent() {
    let fixture = TestRepo::init();
    let tip = fixture.commit("a.txt", "a");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.resolve_commit(&rev("main")).await.unwrap(), Some(tip));
    assert_eq!(repo.resolve_commit(&rev("nope")).await.unwrap(), None);
}

#[tokio::test]
async fn a_repository_without_commits_has_no_head() {
    let fixture = TestRepo::init();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.resolve_commit(&RevName::head()).await.unwrap(), None);
}

#[tokio::test]
async fn a_root_commit_has_no_parent() {
    let fixture = TestRepo::init();
    let root = fixture.commit("a.txt", "a");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(
        repo.resolve_commit(&rev("main").parent()).await.unwrap(),
        None
    );
    assert_eq!(
        repo.resolve_commit(&RevName::of_object(&root))
            .await
            .unwrap(),
        Some(root)
    );
}

#[tokio::test]
async fn the_empty_tree_is_the_well_known_hash() {
    let fixture = TestRepo::init();
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(repo.empty_tree().await.unwrap().as_str(), EMPTY_TREE_SHA1);
}

#[tokio::test]
async fn finds_the_merge_base_and_reports_unrelated_histories_as_absent() {
    let fixture = TestRepo::init();
    let base = fixture.commit("a.txt", "a");
    fixture.git(&["switch", "--quiet", "--create", "feat"]);
    let feat = fixture.commit("b.txt", "b");
    fixture.git(&["switch", "--quiet", "--orphan", "other"]);
    let other = fixture.commit("c.txt", "c");
    let repo = GitRepo::open(fixture.path()).await.unwrap();
    assert_eq!(
        repo.merge_base(&base, &feat).await.unwrap(),
        Some(base.clone())
    );
    assert_eq!(repo.merge_base(&feat, &other).await.unwrap(), None);
}
