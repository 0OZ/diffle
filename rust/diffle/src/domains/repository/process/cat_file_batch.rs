//! One long-lived `git cat-file --batch` per repository. Object names go in on stdin and come
//! back in order on stdout, so a burst of reads (hydrating a diff, sniffing every changed blob)
//! costs one process instead of one each. The process is reaped after a quiet spell and the
//! next read starts a new one; if it dies mid-read, that read fails and the next starts over. A
//! read that is dropped mid-way does the same: its process is discarded, not trusted.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep};

use super::git_error::GitError;
use super::git_object::{GitObject, ObjectKind};
use super::git_runner::{Exit, GitRunner};

const IDLE_REAP_AFTER: Duration = Duration::from_secs(2);
const DISCARD_CHUNK: usize = 64 * 1024;
const MISSING: &str = "missing";
const AMBIGUOUS: &str = "ambiguous";
/// `<sha> <type> <size>`: a name that is missing echoes itself instead, spaces and all.
const HIT_FIELDS: usize = 3;

#[derive(Debug)]
struct Batch {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug)]
struct Slot {
    batch: Option<Batch>,
    last_used: Instant,
    reaper_running: bool,
    /// A request began and has not returned. If it never does (its caller was dropped), git's
    /// reply is still in the pipe and would be read as the answer to the next name.
    unfinished: bool,
}

#[derive(Debug)]
struct Shared {
    slot: Mutex<Slot>,
    spawns: AtomicUsize,
    reap_after: Duration,
}

#[derive(Debug, Clone)]
pub struct ObjectReader {
    cwd: PathBuf,
    shared: Arc<Shared>,
}

impl ObjectReader {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self::reaping_after(cwd, IDLE_REAP_AFTER)
    }

    pub fn reaping_after(cwd: impl Into<PathBuf>, reap_after: Duration) -> Self {
        Self {
            cwd: cwd.into(),
            shared: Arc::new(Shared {
                slot: Mutex::new(Slot {
                    batch: None,
                    last_used: Instant::now(),
                    reaper_running: false,
                    unfinished: false,
                }),
                spawns: AtomicUsize::new(0),
                reap_after,
            }),
        }
    }

    /// How many `cat-file --batch` processes this reader has started. Diagnostics and tests.
    #[must_use]
    pub fn spawns(&self) -> usize {
        self.shared.spawns.load(Ordering::Relaxed)
    }

    /// The object any `cat-file` name picks out (`<sha>`, `<rev>:<path>`), with the first `keep`
    /// bytes of its body; `None` when git reports it missing or ambiguous. A name with a newline
    /// cannot ride the batch and is answered by plain `cat-file` calls.
    pub async fn read(&self, name: &str, keep: usize) -> Result<Option<GitObject>, GitError> {
        if name.contains('\n') {
            return self.read_alone(name, keep).await;
        }
        let mut slot = self.shared.slot.lock().await;
        if slot.unfinished {
            // Its pipe holds an answer nobody read; the process goes, a new one starts below.
            slot.batch = None;
        }
        slot.unfinished = true;
        let result = self.request(&mut slot, name, keep).await;
        slot.unfinished = false;
        slot.last_used = Instant::now();
        match &result {
            Ok(_) => self.keep_reaping(&mut slot),
            Err(_) => slot.batch = None,
        }
        result
    }

    /// Ends the process now instead of after the quiet spell.
    pub async fn shutdown(&self) {
        let batch = self.shared.slot.lock().await.batch.take();
        if let Some(batch) = batch {
            retire(batch).await;
        }
    }

    async fn request(
        &self,
        slot: &mut Slot,
        name: &str,
        keep: usize,
    ) -> Result<Option<GitObject>, GitError> {
        if slot.batch.is_none() {
            slot.batch = Some(self.start()?);
        }
        let Some(batch) = slot.batch.as_mut() else {
            return Err(unexpected("a running cat-file process", String::new()));
        };
        batch
            .stdin
            .write_all(format!("{name}\n").as_bytes())
            .await
            .map_err(GitError::Pipe)?;
        batch.stdin.flush().await.map_err(GitError::Pipe)?;
        let mut header = Vec::new();
        let read = batch
            .stdout
            .read_until(b'\n', &mut header)
            .await
            .map_err(GitError::Pipe)?;
        if read == 0 {
            return Err(unexpected("an object header", "end of output".to_owned()));
        }
        let header = String::from_utf8_lossy(&header)
            .trim_end_matches('\n')
            .to_owned();
        let fields: Vec<&str> = header.split(' ').collect();
        let (Some(&tail), true) = (fields.last(), fields.len() >= HIT_FIELDS) else {
            return Ok(None);
        };
        if tail == MISSING || tail == AMBIGUOUS {
            return Ok(None);
        }
        let (Some(kind), Some(size)) = (
            fields.get(1).and_then(|kind| ObjectKind::parse(kind)),
            tail.parse::<u64>().ok(),
        ) else {
            return Err(unexpected("<sha> <type> <size>", header));
        };
        let head = read_body(&mut batch.stdout, size, keep).await?;
        Ok(Some(GitObject::new(kind, size, head)))
    }

    fn start(&self) -> Result<Batch, GitError> {
        let mut command = Command::new("git");
        command
            .args(["cat-file", "--batch"])
            .current_dir(&self.cwd)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(GitError::Spawn)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(unexpected("piped stdin and stdout", String::new()));
        };
        self.shared.spawns.fetch_add(1, Ordering::Relaxed);
        Ok(Batch {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    fn keep_reaping(&self, slot: &mut Slot) {
        if slot.reaper_running {
            return;
        }
        slot.reaper_running = true;
        let shared = Arc::clone(&self.shared);
        tokio::spawn(async move {
            loop {
                sleep(shared.reap_after).await;
                let mut slot = shared.slot.lock().await;
                if slot.last_used.elapsed() >= shared.reap_after {
                    slot.reaper_running = false;
                    let batch = slot.batch.take();
                    drop(slot);
                    if let Some(batch) = batch {
                        retire(batch).await;
                    }
                    return;
                }
            }
        });
    }

    async fn read_alone(&self, name: &str, keep: usize) -> Result<Option<GitObject>, GitError> {
        let runner = GitRunner::new(&self.cwd);
        let kind = runner.finish(["cat-file", "-t", name]).await?;
        if kind.exit() != Exit::Success {
            return Ok(None);
        }
        let kind_name = kind.into_stdout()?.trimmed();
        let Some(kind) = ObjectKind::parse(&kind_name) else {
            return Err(unexpected("an object type", kind_name));
        };
        let body = runner
            .stdout(["cat-file", kind_name_of(kind), name])
            .await?
            .into_bytes();
        let size = body.len() as u64;
        let mut head = body;
        head.truncate(keep);
        Ok(Some(GitObject::new(kind, size, head)))
    }
}

const fn kind_name_of(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Blob => "blob",
        ObjectKind::Tree => "tree",
        ObjectKind::Commit => "commit",
        ObjectKind::Tag => "tag",
    }
}

fn unexpected(expected: &'static str, output: String) -> GitError {
    GitError::Unexpected { expected, output }
}

/// Reads a body of `size` bytes plus git's trailing newline, keeping only the first `keep`.
async fn read_body(
    stdout: &mut BufReader<ChildStdout>,
    size: u64,
    keep: usize,
) -> Result<Vec<u8>, GitError> {
    let mut head = Vec::new();
    let mut remaining = size;
    let mut chunk = vec![0_u8; DISCARD_CHUNK];
    while remaining > 0 {
        let want = usize::try_from(remaining).map_or(DISCARD_CHUNK, |left| left.min(DISCARD_CHUNK));
        let Some(window) = chunk.get_mut(..want) else {
            return Err(unexpected("a chunk window", String::new()));
        };
        let got = stdout.read(window).await.map_err(GitError::Pipe)?;
        if got == 0 {
            return Err(unexpected(
                "the rest of an object body",
                "end of output".to_owned(),
            ));
        }
        let room = keep.saturating_sub(head.len());
        if let Some(kept) = chunk.get(..got.min(room)) {
            head.extend_from_slice(kept);
        }
        remaining -= got as u64;
    }
    let mut newline = [0_u8; 1];
    stdout
        .read_exact(&mut newline)
        .await
        .map_err(GitError::Pipe)?;
    Ok(head)
}

/// Closing stdin is how `cat-file --batch` is asked to finish.
async fn retire(batch: Batch) {
    let Batch {
        mut child, stdin, ..
    } = batch;
    drop(stdin);
    let _ = child.wait().await;
}

#[cfg(test)]
#[path = "cat_file_batch_tests.rs"]
mod tests;
