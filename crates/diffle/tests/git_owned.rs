//! Owned clones: detached checkout, cat-file shutdown, and the review-state root.
mod common;

use std::path::Path;

use common::{TestRepo, git_in};
use diffle::git::GitRepo;

struct History {
    origin: TestRepo,
    old: String,
    new: String,
}

fn history() -> History {
    let origin = TestRepo::new();
    origin.write("a.txt", "v1\n");
    origin.commit("one");
    let old = origin.git(&["rev-parse", "HEAD"]);
    origin.write("a.txt", "v2\n");
    origin.write("b.txt", "b\n");
    origin.commit("two");
    let new = origin.git(&["rev-parse", "HEAD"]);
    History { origin, old, new }
}

async fn clone_of(origin: &TestRepo) -> (tempfile::TempDir, GitRepo) {
    let dir = tempfile::tempdir().unwrap();
    let repo = GitRepo::clone_into(origin.path.to_str().unwrap(), dir.path()).await.unwrap();
    (dir, repo)
}

fn dirty_origin(origin: &TestRepo) -> (String, String) {
    origin.write("a.txt", "dirty\n");
    origin.write("u.txt", "u\n");
    (origin.git(&["rev-parse", "HEAD"]), origin.git(&["status", "--porcelain"]))
}

#[tokio::test]
async fn clone_checkout_updates_only_its_own_head_and_files() {
    let h = history();
    let (_dir, repo) = clone_of(&h.origin).await;
    let (head, status) = dirty_origin(&h.origin);
    assert!(repo.is_isolated());
    assert_eq!(repo.review_state_dir(), repo.git_dir.as_path());
    repo.checkout_owned(&h.old).await.unwrap();
    assert_eq!(repo.resolve("HEAD").await.unwrap(), h.old);
    assert!(repo.branch_ref("HEAD").await.unwrap().is_none(), "owned checkout must detach");
    assert_eq!(std::fs::read_to_string(repo.root.join("a.txt")).unwrap(), "v1\n");
    assert!(!repo.root.join("b.txt").exists(), "files absent from the commit must leave the clone");
    assert_eq!(h.origin.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(h.origin.git(&["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(h.origin.git(&["status", "--porcelain"]), status);
    assert_eq!(std::fs::read_to_string(h.origin.path.join("a.txt")).unwrap(), "dirty\n");
    assert_eq!(std::fs::read_to_string(h.origin.path.join("u.txt")).unwrap(), "u\n");
}

// Exercise the minimum-supported Git family without changing PATH for parallel tests.
#[cfg(target_os = "linux")]
#[test]
fn owned_checkout_with_system_git() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "clone_checkout_updates_only_its_own_head_and_files", "--nocapture"])
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "system git checkout failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn branch_name_checkout_detaches_without_moving_the_branch() {
    let h = history();
    let (dir, repo) = clone_of(&h.origin).await;
    let tip = git_in(dir.path(), &["rev-parse", "refs/heads/main"]);
    repo.checkout_owned("main").await.unwrap();
    assert_eq!(repo.resolve("HEAD").await.unwrap(), h.new);
    assert!(repo.branch_ref("HEAD").await.unwrap().is_none(), "a branch name must not attach HEAD");
    assert_eq!(git_in(dir.path(), &["rev-parse", "refs/heads/main"]), tip);
}

#[tokio::test]
async fn ordinary_repo_checkout_is_rejected() {
    let h = history();
    let (head, status) = dirty_origin(&h.origin);
    let repo = h.origin.open().await;
    assert!(!repo.is_isolated());
    assert!(repo.checkout_owned(&h.old).await.is_err());
    assert_eq!(h.origin.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(h.origin.git(&["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(h.origin.git(&["status", "--porcelain"]), status);
    assert_eq!(std::fs::read_to_string(h.origin.path.join("a.txt")).unwrap(), "dirty\n");
    assert_eq!(std::fs::read_to_string(h.origin.path.join("u.txt")).unwrap(), "u\n");
}

#[tokio::test]
async fn unknown_target_is_rejected_without_changing_head() {
    let h = history();
    let (dir, repo) = clone_of(&h.origin).await;
    let head = git_in(dir.path(), &["rev-parse", "HEAD"]);
    assert!(repo.checkout_owned("no-such-rev").await.is_err());
    assert_eq!(git_in(dir.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git_in(dir.path(), &["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(std::fs::read_to_string(dir.path().join("a.txt")).unwrap(), "v2\n");
}

#[tokio::test]
async fn newline_in_target_is_rejected_without_changing_head_or_refs() {
    let h = history();
    let (dir, repo) = clone_of(&h.origin).await;
    let head = git_in(dir.path(), &["rev-parse", "HEAD"]);
    let refs = git_in(dir.path(), &["for-each-ref", "--format=%(refname)"]);
    assert!(repo.checkout_owned(&format!("{}\n--branch\nmain", h.old)).await.is_err());
    assert_eq!(git_in(dir.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git_in(dir.path(), &["for-each-ref", "--format=%(refname)"]), refs);
}

#[cfg(target_os = "linux")]
fn batch_pids(root: &Path) -> Vec<i32> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
        let parts: Vec<&str> = cmd
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .filter_map(|part| std::str::from_utf8(part).ok())
            .collect();
        let is_batch = parts.windows(2).any(|pair| pair == ["cat-file", "--batch"]);
        let cwd = std::fs::read_link(format!("/proc/{pid}/cwd")).ok();
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        if is_batch && cwd.as_deref() == Some(root.as_path()) {
            found.push(pid);
        }
    }
    found
}

#[tokio::test]
async fn close_reaps_cat_file_before_the_clone_directory_is_removed() {
    let h = history();
    let (dir, repo) = clone_of(&h.origin).await;
    assert_eq!(repo.show("HEAD", "a.txt").await.unwrap().unwrap(), b"v2\n");
    #[cfg(target_os = "linux")]
    assert!(!batch_pids(dir.path()).is_empty(), "cat-file --batch should be running");
    repo.close().await;
    #[cfg(target_os = "linux")]
    assert!(batch_pids(dir.path()).is_empty(), "close returned before cat-file exited");
    let err = repo.show("HEAD", "a.txt").await.unwrap_err();
    assert!(err.to_string().contains("closed"), "{err}");
    assert_eq!(repo.cat_file_spawns(), 1, "a closed read must not start another batch");
    std::fs::remove_dir_all(dir.path()).unwrap();
}

#[tokio::test]
async fn idle_timeout_after_close_does_not_respawn_cat_file() {
    let h = history();
    let (_dir, repo) = clone_of(&h.origin).await;
    assert!(repo.show("HEAD", "a.txt").await.unwrap().is_some());
    assert_eq!(repo.cat_file_spawns(), 1);
    repo.close().await;
    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
    let err = repo.show("HEAD", "a.txt").await.unwrap_err();
    assert!(err.to_string().contains("closed"), "{err}");
    assert_eq!(repo.cat_file_spawns(), 1);
}

#[tokio::test]
async fn review_state_dir_defaults_to_git_dir_until_set() {
    let h = history();
    let mut repo = h.origin.open().await;
    assert_eq!(repo.review_state_dir(), repo.git_dir.as_path());
    let git_dir = repo.git_dir.clone();
    let custom = h.origin.path.join("durable-state");
    repo.set_review_state_dir(custom.clone());
    assert_eq!(repo.review_state_dir(), custom.as_path());
    assert_eq!(repo.git_dir, git_dir);
    assert_eq!(repo.resolve("HEAD").await.unwrap(), h.new);
}
