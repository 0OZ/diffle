//! Ctrl+C while Git is stuck: the review ends within its grace period, and a second Ctrl+C ends
//! it at once. Real signals sent to a real server process whose `git diff` never returns.

#![cfg(unix)]

use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process};
use tempfile::TempDir;

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const STARTUP: Duration = Duration::from_secs(30);
const STUCK: Duration = Duration::from_secs(15);
/// The server gives work 3 s to settle after the first signal; this leaves room for the rest.
const ENDS_WITHIN: Duration = Duration::from_secs(8);
const ENDS_AT_ONCE: Duration = Duration::from_secs(2);
/// The first signal must be heard, and the wait for a second one polled, before the second is sent.
const BETWEEN_SIGNALS: Duration = Duration::from_millis(600);

/// Runs real Git until the flag file exists, then hangs on every `git diff`.
const SHIM: &str = r#"#!/bin/sh
if [ -e "$SHIM_FLAG" ]; then
  for arg in "$@"; do
    if [ "$arg" = diff ]; then : > "$SHIM_MARKER"; exec sleep 600; fi
  done
fi
exec "$REAL_GIT" "$@"
"#;

struct Review {
    child: Child,
    repo: PathBuf,
    flag: PathBuf,
    marker: PathBuf,
    lines: Receiver<String>,
    _scratch: TempDir,
}

impl Drop for Review {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn git(repo: &Path, args: &[&str]) -> Result {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(repo)
        .status()?;
    status.success().then_some(()).ok_or("git failed".into())
}

fn real_git() -> Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or("no PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("git"))
        .find(|candidate| candidate.is_file())
        .ok_or("no git on PATH".into())
}

impl Review {
    /// A review of the working tree, started with the shim first on `PATH` (not yet blocking).
    fn start() -> Result<Self> {
        let scratch = tempfile::tempdir()?;
        let (repo, shim_dir, client) = (
            scratch.path().join("repo"),
            scratch.path().join("bin"),
            scratch.path().join("client"),
        );
        for dir in [&repo, &shim_dir, &client] {
            fs::create_dir(dir)?;
        }
        fs::write(client.join("index.html"), "<!doctype html>")?;
        git(&repo, &["init", "-q", "-b", "main"])?;
        fs::write(repo.join("a.txt"), "one\n")?;
        git(&repo, &["add", "."])?;
        git(&repo, &["commit", "-q", "-m", "one"])?;
        fs::write(repo.join("a.txt"), "two\n")?;

        let shim = shim_dir.join("git");
        fs::write(&shim, SHIM)?;
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))?;
        let path = std::env::join_paths(std::iter::once(shim_dir).chain(std::env::split_paths(
            &std::env::var_os("PATH").ok_or("no PATH")?,
        )))?;
        let (flag, marker) = (scratch.path().join("block"), scratch.path().join("blocked"));

        let mut child = Command::new(env!("CARGO_BIN_EXE_diffle"))
            .args(["-p", "0", "--no-open", "--no-lsp", "--client-dir"])
            .arg(&client)
            .arg("-C")
            .arg(&repo)
            .env("PATH", path)
            .env("REAL_GIT", real_git()?)
            .env("SHIM_FLAG", &flag)
            .env("SHIM_MARKER", &marker)
            .stderr(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()?;
        let stderr = child.stderr.take().ok_or("no stderr")?;
        let (send, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        let review = Self {
            child,
            repo,
            flag,
            marker,
            lines,
            _scratch: scratch,
        };
        review.wait_for_line("watching the worktree")?;
        Ok(review)
    }

    fn wait_for_line(&self, wanted: &str) -> Result {
        let deadline = Instant::now() + STARTUP;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if self.lines.recv_timeout(left)?.contains(wanted) {
                return Ok(());
            }
        }
        Err(format!("never saw {wanted:?}").into())
    }

    /// Edits a file so the watcher recomputes, and waits until that recomputation hangs in Git.
    fn get_stuck(&self) -> Result {
        fs::write(&self.flag, "")?;
        fs::write(self.repo.join("a.txt"), "three\n")?;
        let deadline = Instant::now() + STUCK;
        while !self.marker.exists() {
            if Instant::now() > deadline {
                return Err("the recomputation never reached git diff".into());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    fn interrupt(&self) -> Result {
        let pid = Pid::from_raw(i32::try_from(self.child.id())?).ok_or("no pid")?;
        kill_process(pid, Signal::INT)?;
        Ok(())
    }

    /// The exit status, or `None` if the server was still running after `limit`.
    fn exit_within(&mut self, limit: Duration) -> Result<Option<ExitStatus>> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(Some(status));
            }
            if Instant::now() > deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn ctrl_c_ends_a_review_whose_git_is_stuck() -> Result {
    let mut review = Review::start()?;
    review.get_stuck()?;
    review.interrupt()?;
    let status = review.exit_within(ENDS_WITHIN)?;
    assert!(status.is_some_and(|status| status.success()), "{status:?}");
    Ok(())
}

#[test]
fn a_second_ctrl_c_does_not_wait_for_the_grace_period() -> Result {
    let mut review = Review::start()?;
    review.get_stuck()?;
    review.interrupt()?;
    thread::sleep(BETWEEN_SIGNALS);
    // The first signal alone leaves the stuck work its grace period.
    assert_eq!(
        review.child.try_wait()?,
        None,
        "ended before the second signal"
    );
    review.interrupt()?;
    let status = review.exit_within(ENDS_AT_ONCE)?;
    assert!(
        status.is_some_and(|status| status.success()),
        "{status:?} {ENDS_AT_ONCE:?} after the second signal"
    );
    Ok(())
}
