//! A throwaway real Git repository and the timing helpers the watcher tests share.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

use super::error::WatchError;
use super::scope::watch_target::{IgnoreRule, WatchTarget};
use super::watcher::FsWatcher;

/// Short, so a test is fast; long enough to fold a burst of writes.
pub const DEBOUNCE: Duration = Duration::from_millis(30);
/// Generous upper bound for a signal that must arrive.
pub const SIGNAL_WITHIN: Duration = Duration::from_secs(5);
/// Many debounce periods: a signal that was going to arrive would have.
pub const QUIET_FOR: Duration = Duration::from_millis(300);

/// The paths an owner has put on the ignore list, which it may edit while a watcher runs.
pub type Listed = Arc<RwLock<HashSet<String>>>;

/// A rule over a shared list, as a session would hand over: the path or a listed ancestor.
pub fn rule_over(listed: &Listed) -> IgnoreRule {
    let listed = Arc::clone(listed);
    Arc::new(move |rel| {
        let listed = listed.read().unwrap();
        rel.match_indices('/')
            .map(|(at, _)| &rel[..at])
            .chain([rel])
            .any(|candidate| listed.contains(candidate))
    })
}

pub struct Repo {
    dir: TempDir,
}

impl Repo {
    pub fn init() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "--quiet", "--initial-branch", "main"]);
        repo
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn git_dir(&self) -> PathBuf {
        self.root().join(".git")
    }

    /// Runs Git in the repository, isolated from the machine's configuration.
    pub fn git(&self, args: &[&str]) -> String {
        run_git(self.root(), args)
    }

    pub fn commit_file(&self, name: &str, content: &str) {
        fs::write(self.root().join(name), content).unwrap();
        self.git(&["add", name]);
        self.git(&["commit", "--quiet", "-m", name]);
    }
}

pub fn run_git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Replaces `path` the way Git does: write a sibling, rename it over.
pub fn replace_atomically(path: &Path, content: impl AsRef<[u8]>) {
    let mut sibling = path.as_os_str().to_owned();
    sibling.push(".tmp-replace");
    let sibling = PathBuf::from(sibling);
    fs::write(&sibling, content).unwrap();
    fs::rename(&sibling, path).unwrap();
}

pub async fn expect_signal(signals: &mut Receiver<()>) {
    let received = timeout(SIGNAL_WITHIN, signals.recv()).await;
    assert!(
        matches!(received, Ok(Some(()))),
        "no signal arrived: {received:?}"
    );
}

pub async fn expect_quiet(signals: &mut Receiver<()>) {
    let received = timeout(QUIET_FOR, signals.recv()).await;
    assert!(received.is_err(), "unexpected signal: {received:?}");
}

pub async fn watch_worktree(repo: &Repo, listed: &Listed) -> (FsWatcher, Receiver<()>) {
    let target = WatchTarget::Worktree {
        root: repo.root().into(),
        git_dir: repo.git_dir(),
        common_dir: repo.git_dir(),
        ignored: rule_over(listed),
    };
    FsWatcher::start(target, DEBOUNCE).await.unwrap()
}

pub async fn watch_refs_result(repo: &Repo) -> Result<(FsWatcher, Receiver<()>), WatchError> {
    let target = WatchTarget::Refs {
        git_dir: repo.git_dir(),
        common_dir: repo.git_dir(),
    };
    FsWatcher::start(target, DEBOUNCE).await
}

pub async fn watch_refs(repo: &Repo) -> (FsWatcher, Receiver<()>) {
    watch_refs_result(repo).await.unwrap()
}

/// One signal, then quiet: lets a test prove the next signal came from its next step.
pub async fn settle(signals: &mut Receiver<()>) {
    expect_signal(signals).await;
    expect_quiet(signals).await;
}
