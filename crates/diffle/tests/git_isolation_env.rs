//! Repository-routing environments belong to the caller, never to an owned clone.
mod common;

use std::path::Path;
use std::process::Command;

use common::TestRepo;
use diffle::git::{GitRepo, GrepOptions};

fn run_clone_child(origin: &TestRepo, clone: &Path, old: &str, env: &[(&str, &Path)]) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "clone_child", "--nocapture"])
        .env("DIFFLE_TEST_ORIGIN", &origin.path)
        .env("DIFFLE_TEST_CLONE", clone)
        .env("DIFFLE_TEST_OLD", old)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated clone child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn history() -> (TestRepo, String) {
    let origin = TestRepo::new();
    origin.write("a.txt", "old\n");
    #[cfg(unix)] // Windows cannot express a newline in a filename.
    origin.write("line\nbreak.txt", "old newline\n");
    origin.commit("old");
    let old = origin.git(&["rev-parse", "HEAD"]);
    origin.write("a.txt", "new\n");
    #[cfg(unix)]
    origin.write("line\nbreak.txt", "new newline\n");
    origin.commit("new");
    (origin, old)
}

#[test]
fn inherited_index_is_preserved_byte_for_byte() {
    let (origin, old) = history();
    origin.write("a.txt", "staged caller content\n");
    origin.git(&["add", "a.txt"]);
    origin.write("a.txt", "unstaged caller content\n");
    let index = origin.path.join(".git/index");
    let before = std::fs::read(&index).unwrap();
    let clone = tempfile::tempdir().unwrap();
    run_clone_child(&origin, clone.path(), &old, &[("GIT_INDEX_FILE", &index)]);
    assert_eq!(std::fs::read(&index).unwrap(), before, "owned clone overwrote the caller's index");
    assert_eq!(origin.git(&["show", ":a.txt"]), "staged caller content");
    assert_eq!(std::fs::read_to_string(origin.path.join("a.txt")).unwrap(), "unstaged caller content\n");
}

#[test]
fn inherited_git_dir_alone_cannot_grant_ownership_over_the_caller() {
    let (origin, old) = history();
    let clone = tempfile::tempdir().unwrap();
    run_clone_child(&origin, clone.path(), &old, &[("GIT_DIR", &origin.path.join(".git"))]);
}

#[test]
fn inherited_git_dir_and_work_tree_cannot_redirect_owned_clone() {
    let (origin, old) = history();
    let index = std::fs::read(origin.path.join(".git/index")).unwrap();
    let head = origin.git(&["rev-parse", "HEAD"]);
    let clone = tempfile::tempdir().unwrap();
    run_clone_child(
        &origin,
        clone.path(),
        &old,
        &[("GIT_DIR", &origin.path.join(".git")), ("GIT_WORK_TREE", &origin.path)],
    );
    assert_eq!(origin.git(&["rev-parse", "HEAD"]), head, "owned checkout moved the caller's HEAD");
    assert_eq!(std::fs::read(origin.path.join(".git/index")).unwrap(), index);
    assert_eq!(std::fs::read_to_string(origin.path.join("a.txt")).unwrap(), "new\n");
}

#[test]
fn inherited_object_and_common_directories_cannot_route_clone_storage() {
    let (origin, old) = history();
    let clone = tempfile::tempdir().unwrap();
    run_clone_child(
        &origin,
        clone.path(),
        &old,
        &[
            ("GIT_COMMON_DIR", &origin.path.join(".git")),
            ("GIT_OBJECT_DIRECTORY", &origin.path.join(".git/objects")),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", &origin.path.join(".git/objects")),
        ],
    );
    assert!(clone.path().join(".git/objects").is_dir(), "clone must own its object database");
}

#[tokio::test]
#[ignore = "entry point for command-local environment tests"]
async fn clone_child() {
    let origin = std::env::var_os("DIFFLE_TEST_ORIGIN").unwrap();
    let clone = std::env::var_os("DIFFLE_TEST_CLONE").unwrap();
    let old = std::env::var("DIFFLE_TEST_OLD").unwrap();
    let clone = Path::new(&clone);
    let repo = GitRepo::clone_into(Path::new(&origin).to_str().unwrap(), clone).await.unwrap();
    let root = dunce::canonicalize(clone).unwrap();
    assert_eq!(repo.root, root, "owned clone opened the caller's worktree");
    assert!(repo.git_dir.starts_with(&root), "owned clone opened an external git dir");
    assert!(repo.common_dir.starts_with(&root), "owned clone opened an external common dir");
    repo.checkout_owned(&old).await.unwrap();
    assert_eq!(repo.resolve("HEAD").await.unwrap(), old, "owned exec was redirected");
    assert_eq!(repo.show("HEAD", "a.txt").await.unwrap().unwrap(), b"old\n", "batch read was redirected");
    let hits = repo.grep("old", "HEAD", 10, GrepOptions::default()).await.unwrap();
    assert!(hits.matches.iter().any(|hit| hit.path == "a.txt"), "streaming grep was redirected");
    #[cfg(unix)]
    assert_eq!(
        repo.show("HEAD", "line\nbreak.txt").await.unwrap().unwrap(),
        b"old newline\n",
        "one-shot newline read was redirected"
    );
    repo.close().await;
}

#[test]
fn isolated_submodule_probe_reads_the_submodules_head() {
    let submodule = TestRepo::new();
    submodule.write("sub.txt", "old submodule\n");
    submodule.commit("old submodule");
    let old = submodule.git(&["rev-parse", "HEAD"]);
    submodule.write("sub.txt", "new submodule\n");
    submodule.commit("new submodule");
    let new = submodule.git(&["rev-parse", "HEAD"]);
    let origin = TestRepo::new();
    origin.git(&["update-index", "--add", "--cacheinfo", &format!("160000,{old},sub")]);
    origin.git(&["commit", "-qm", "gitlink"]);
    let clone = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "submodule_child", "--nocapture"])
        .env("DIFFLE_TEST_ORIGIN", &origin.path)
        .env("DIFFLE_TEST_CLONE", clone.path())
        .env("DIFFLE_TEST_SUBMODULE", submodule.path.join(".git"))
        .env("DIFFLE_TEST_SUBMODULE_HEAD", new)
        .env("GIT_DIR", origin.path.join(".git"))
        .env("GIT_WORK_TREE", &origin.path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "submodule child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "entry point for command-local environment tests"]
async fn submodule_child() {
    let origin = std::env::var("DIFFLE_TEST_ORIGIN").unwrap();
    let clone = std::env::var_os("DIFFLE_TEST_CLONE").unwrap();
    let submodule = std::env::var("DIFFLE_TEST_SUBMODULE").unwrap();
    let expected = std::env::var("DIFFLE_TEST_SUBMODULE_HEAD").unwrap();
    let repo = GitRepo::clone_into(&origin, Path::new(&clone)).await.unwrap();
    // A real gitfile locates an already checked-out submodule, without network fetching.
    std::fs::write(repo.root.join("sub/.git"), format!("gitdir: {submodule}\n")).unwrap();
    let files = repo.numstat("HEAD", diffle::git::WORKTREE).await.unwrap();
    let sub = files.iter().find(|file| file.path == "sub").expect("changed submodule");
    assert_eq!(sub.blob, expected, "submodule probe inherited the caller's repository");
    repo.close().await;
}

#[test]
fn ordinary_open_keeps_inherited_repository_routing() {
    let (origin, _) = history();
    let other = TestRepo::new();
    other.write("other.txt", "other\n");
    other.commit("other");
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "ordinary_child", "--nocapture"])
        .env("DIFFLE_TEST_ORIGIN", &origin.path)
        .env("DIFFLE_TEST_OTHER", &other.path)
        .env("GIT_DIR", origin.path.join(".git"))
        .env("GIT_WORK_TREE", &origin.path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
}

#[tokio::test]
#[ignore = "entry point for command-local environment tests"]
async fn ordinary_child() {
    let origin = std::env::var_os("DIFFLE_TEST_ORIGIN").unwrap();
    let other = std::env::var_os("DIFFLE_TEST_OTHER").unwrap();
    let repo = GitRepo::open(Path::new(&other)).await.unwrap();
    assert_eq!(repo.root, Path::new(&origin));
    assert!(!repo.is_isolated());
    assert_eq!(repo.show("HEAD", "a.txt").await.unwrap().unwrap(), b"new\n");
    repo.close().await;
}
