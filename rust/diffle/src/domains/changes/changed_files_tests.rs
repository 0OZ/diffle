use super::*;
use crate::domains::changes::changed_file::Delta;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::test_repo::TestRepo;

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

fn object(id: &ObjectId) -> TreeSource {
    TreeSource::Object(id.clone())
}

fn summary(files: &[ChangedFile]) -> Vec<(char, &str)> {
    files
        .iter()
        .map(|file| (file.kind.code(), file.path.as_str()))
        .collect()
}

fn blob(fixture: &TestRepo, spec: &str) -> Option<ObjectId> {
    Some(ObjectId::parse(&fixture.git(&["rev-parse", spec])).unwrap())
}

fn text(additions: u32, deletions: u32) -> Delta {
    Delta::Text {
        additions,
        deletions,
    }
}

#[tokio::test]
async fn compares_two_commits_with_counts_blobs_and_renames() {
    let fixture = TestRepo::init();
    fixture.commit("edit.txt", "one\ntwo\n");
    fixture.commit(
        "rename-me.txt",
        "a long enough body\nto be recognised\nas the same file\n",
    );
    fixture.commit("delete.txt", "bye\n");
    let base = fixture.commit("binary.bin", "x");
    fixture.git(&["mv", "rename-me.txt", "renamed.txt"]);
    fixture.git(&["rm", "--quiet", "delete.txt"]);
    std::fs::write(fixture.path().join("edit.txt"), "one\nthree\nfour\n").unwrap();
    std::fs::write(fixture.path().join("added.txt"), "new\n").unwrap();
    std::fs::write(fixture.path().join("binary.bin"), b"\0\x01\x02").unwrap();
    fixture.git(&["add", "--all"]);
    fixture.git(&["commit", "--quiet", "--message", "change"]);
    let tip = ObjectId::parse(&fixture.git(&["rev-parse", "HEAD"])).unwrap();
    let repo = open(&fixture).await;
    let files = changed_files(&repo, &object(&base), &object(&tip))
        .await
        .unwrap();
    assert_eq!(
        summary(&files),
        [
            ('A', "added.txt"),
            ('M', "binary.bin"),
            ('D', "delete.txt"),
            ('M', "edit.txt"),
            ('R', "renamed.txt"),
        ]
    );
    let [added, binary, deleted, edit, renamed] = &files[..] else {
        unreachable!("five files were asserted above");
    };
    assert_eq!(added.delta, text(1, 0));
    assert_eq!(added.blob, blob(&fixture, "HEAD:added.txt"));
    assert_eq!(added.old_blob, None);
    assert!(binary.is_binary());
    assert_eq!(deleted.blob, None);
    assert_eq!(deleted.delta, text(0, 1));
    assert_eq!(edit.delta, text(2, 1));
    assert_eq!(edit.old_blob, blob(&fixture, &format!("{base}:edit.txt")));
    assert_eq!(renamed.old_path().as_str(), "rename-me.txt");
}

#[tokio::test]
async fn the_worktree_side_includes_untracked_files_and_hashes_them_from_disk() {
    let fixture = TestRepo::init();
    let base = fixture.commit("tracked.txt", "one\n");
    std::fs::write(fixture.path().join("tracked.txt"), "one\ntwo\n").unwrap();
    std::fs::write(fixture.path().join("untracked.txt"), "u1\nu2\nu3").unwrap();
    std::fs::write(fixture.path().join("staged.txt"), "s\n").unwrap();
    fixture.git(&["add", "staged.txt"]);
    let repo = open(&fixture).await;
    let files = changed_files(&repo, &object(&base), &TreeSource::Worktree)
        .await
        .unwrap();
    assert_eq!(
        summary(&files),
        [
            ('A', "staged.txt"),
            ('M', "tracked.txt"),
            ('A', "untracked.txt")
        ]
    );
    for file in &files {
        assert_eq!(
            file.blob,
            blob(&fixture, &fixture.git(&["hash-object", file.path.as_str()])),
            "{}",
            file.path
        );
    }
    assert_eq!(files[2].delta, text(3, 0));
    assert_eq!(
        files[1].old_blob,
        blob(&fixture, &format!("{base}:tracked.txt"))
    );
}

#[tokio::test]
async fn with_the_worktree_as_the_old_side_everything_reads_backwards() {
    let fixture = TestRepo::init();
    let head = fixture.commit("tracked.txt", "committed\n");
    std::fs::write(fixture.path().join("tracked.txt"), "edited\nlonger\n").unwrap();
    std::fs::write(fixture.path().join("untracked.txt"), "u\n").unwrap();
    let repo = open(&fixture).await;
    let files = changed_files(&repo, &TreeSource::Worktree, &object(&head))
        .await
        .unwrap();
    assert_eq!(
        summary(&files),
        [('M', "tracked.txt"), ('D', "untracked.txt")]
    );
    let [tracked, untracked] = &files[..] else {
        unreachable!("two files were asserted above");
    };
    assert_eq!(tracked.blob, blob(&fixture, "HEAD:tracked.txt"));
    assert_eq!(
        tracked.old_blob,
        blob(&fixture, &fixture.git(&["hash-object", "tracked.txt"]))
    );
    assert_eq!(tracked.delta, text(1, 2));
    assert_eq!(untracked.delta, text(0, 1));
    assert_eq!(untracked.blob, None);
    assert_eq!(
        untracked.old_blob,
        blob(&fixture, &fixture.git(&["hash-object", "untracked.txt"]))
    );
}

#[tokio::test]
async fn the_worktree_against_itself_changes_nothing() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "a\n");
    std::fs::write(fixture.path().join("b.txt"), "b\n").unwrap();
    let repo = open(&fixture).await;
    let files = changed_files(&repo, &TreeSource::Worktree, &TreeSource::Worktree)
        .await
        .unwrap();
    assert!(files.is_empty());
}

#[tokio::test]
async fn an_unborn_branch_shows_every_file_as_added() {
    let fixture = TestRepo::init();
    std::fs::write(fixture.path().join("a.txt"), "a\n").unwrap();
    let repo = open(&fixture).await;
    let empty = TreeSource::Object(repo.empty_tree().await.unwrap());
    let files = changed_files(&repo, &empty, &TreeSource::Worktree)
        .await
        .unwrap();
    assert_eq!(summary(&files), [('A', "a.txt")]);
}

#[tokio::test]
async fn paths_that_look_like_patterns_or_contain_odd_characters_survive() {
    let fixture = TestRepo::init();
    let base = fixture.commit("seed.txt", "s\n");
    for name in ["[id].tsx", "with space.txt", "tab\there.txt", "ünï.txt"] {
        std::fs::write(fixture.path().join(name), "x\n").unwrap();
    }
    fixture.git(&["add", "--all"]);
    fixture.git(&["commit", "--quiet", "--message", "odd"]);
    let tip = ObjectId::parse(&fixture.git(&["rev-parse", "HEAD"])).unwrap();
    let repo = open(&fixture).await;
    let files = changed_files(&repo, &object(&base), &object(&tip))
        .await
        .unwrap();
    let mut names: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["[id].tsx", "tab\there.txt", "with space.txt", "ünï.txt"]
    );
    assert!(files.iter().all(|file| file.delta == text(1, 0)));
}
