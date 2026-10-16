use std::fs::{File, OpenOptions};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::*;
use crate::domains::repository::test_repo::TestRepo;

/// A guard so a missing deadline fails the test instead of hanging it.
const NEVER_RETURNED: Duration = Duration::from_secs(10);

#[test]
fn the_default_deadline_is_the_generous_one() {
    assert_eq!(GitRunner::new(".").deadline, FINISH_DEADLINE);
}

#[tokio::test]
async fn a_command_that_ends_in_time_is_not_disturbed() {
    let fixture = TestRepo::init();
    let runner = GitRunner::with_deadline(fixture.path(), Duration::from_secs(60));
    let finished = runner.finish(["--version"]).await.unwrap();
    assert_eq!(finished.exit(), Exit::Success);
}

/// `git diff --no-index` opens a named pipe and waits for a writer, which makes a Git that
/// never ends without needing a stalled mount.
#[cfg(unix)]
#[tokio::test]
async fn a_command_that_never_ends_is_killed_at_the_deadline() {
    let fixture = TestRepo::init();
    let pipe = fixture.path().join("blocked.pipe");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap()
            .success()
    );
    let deadline = Duration::from_millis(300);
    let runner = GitRunner::with_deadline(fixture.path(), deadline);
    let started = Instant::now();

    let outcome = tokio::time::timeout(
        NEVER_RETURNED,
        runner.finish(["diff", "--no-index", "--", "/dev/null", "blocked.pipe"]),
    )
    .await;

    let Ok(result) = outcome else {
        panic!("finish was still waiting after {NEVER_RETURNED:?}");
    };
    let Err(GitError::TimedOut { command, after }) = result else {
        panic!("expected a timeout, got {result:?}");
    };
    assert_eq!((command.contains("blocked.pipe"), after), (true, deadline));
    assert!(started.elapsed() >= deadline);
    assert_nobody_reads(&pipe).await;
}

/// Opening a named pipe for writing waits for a reader, so a writer that gets through means
/// Git is still alive. Unblocks the probe afterwards either way.
#[cfg(unix)]
async fn assert_nobody_reads(pipe: &std::path::Path) {
    // A killed process leaves the kernel a moment to tear it down.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (opened, writer_in) = mpsc::channel();
    let target = pipe.to_owned();
    let probe = std::thread::spawn(move || {
        let writer = OpenOptions::new().write(true).open(target);
        let _ = opened.send(());
        writer
    });
    let got_through = writer_in.recv_timeout(Duration::from_millis(500)).is_ok();
    // Releases a probe still waiting for a reader.
    drop(File::open(pipe));
    probe.join().unwrap().unwrap();
    assert!(!got_through, "the command is still running");
}
