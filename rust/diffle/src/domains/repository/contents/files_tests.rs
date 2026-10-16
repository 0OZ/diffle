use super::*;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

#[tokio::test]
async fn reads_a_file_from_a_commit_and_from_the_worktree() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "committed\n");
    std::fs::write(fixture.path().join("a.txt"), "edited\n").unwrap();
    let repo = open(&fixture).await;
    let at_commit = repo
        .read_file(&TreeSource::Object(commit), &path("a.txt"))
        .await
        .unwrap();
    assert_eq!(at_commit.as_deref(), Some(b"committed\n".as_slice()));
    let on_disk = repo
        .read_file(&TreeSource::Worktree, &path("a.txt"))
        .await
        .unwrap();
    assert_eq!(on_disk.as_deref(), Some(b"edited\n".as_slice()));
}

#[tokio::test]
async fn absent_files_directories_and_missing_parents_read_as_none() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("dir/a.txt", "x\n");
    let repo = open(&fixture).await;
    for source in [TreeSource::Worktree, TreeSource::Object(commit)] {
        for missing in ["nope.txt", "dir", "dir/a.txt/below"] {
            assert_eq!(repo.read_file(&source, &path(missing)).await.unwrap(), None);
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_reads_as_its_target_text_not_the_target() {
    let fixture = TestRepo::init();
    std::fs::write(fixture.path().join("secret.txt"), "secret\n").unwrap();
    std::os::unix::fs::symlink("secret.txt", fixture.path().join("link")).unwrap();
    let repo = open(&fixture).await;
    let read = repo
        .read_file(&TreeSource::Worktree, &path("link"))
        .await
        .unwrap();
    assert_eq!(read.as_deref(), Some(b"secret.txt".as_slice()));
    assert_eq!(
        repo.file_head(&TreeSource::Worktree, &path("link"), 4)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn heads_are_bounded_on_both_sides() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "0123456789\n");
    let repo = open(&fixture).await;
    for source in [TreeSource::Worktree, TreeSource::Object(commit)] {
        let head = repo.file_head(&source, &path("a.txt"), 4).await.unwrap();
        assert_eq!(head.as_deref(), Some(b"0123".as_slice()));
    }
}

#[tokio::test]
async fn blob_heads_skip_what_is_not_a_blob() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "abcdef\n");
    let blob = fixture.git(&["rev-parse", "HEAD:a.txt"]);
    let blob = ObjectId::parse(blob.trim()).unwrap();
    let repo = open(&fixture).await;
    let heads = repo.blob_heads(&[blob.clone(), commit], 3).await.unwrap();
    assert_eq!(heads.len(), 1);
    assert_eq!(heads.get(&blob).map(Vec::as_slice), Some(b"abc".as_slice()));
}
