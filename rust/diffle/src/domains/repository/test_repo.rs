//! A throwaway real Git repository for tests, isolated from the machine's Git config.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::names::object_id::ObjectId;

const DEFAULT_BRANCH: &str = "main";

pub struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    /// An empty repository on `main`, without commits.
    pub fn init() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "--quiet", "--initial-branch", DEFAULT_BRANCH]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs Git here, panicking on failure, and returns its trimmed stdout.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(self.path())
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

    /// Commits `file` with `content` on the current branch.
    pub fn commit(&self, file: &str, content: &str) -> ObjectId {
        let target = self.path().join(file);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(target, content).unwrap();
        self.git(&["add", "--", file]);
        self.git(&["commit", "--quiet", "--message", &format!("edit {file}")]);
        ObjectId::parse(&self.git(&["rev-parse", "HEAD"])).unwrap()
    }
}
