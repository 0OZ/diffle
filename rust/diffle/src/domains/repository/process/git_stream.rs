//! Reading a Git command's output as it arrives, so a caller that has seen enough can stop
//! the command instead of buffering everything it would still print.

use std::ffi::OsStr;
use std::io;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStdout};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::git_error::GitError;
use super::git_runner::{Exit, GitRunner};

const READ_CHUNK: usize = 64 * 1024;
const NUL: u8 = 0;

/// How a stream of records is cut, bounded and judged.
#[derive(Debug, Clone, Copy)]
pub struct RecordStream<'a> {
    /// The byte that ends a record.
    pub terminator: u8,
    /// NUL-terminated fields a record starts with. The terminator ends a record only after
    /// them: `git grep -z` prints `path NUL line NUL text`, and a path may hold a newline.
    pub leading_fields: usize,
    /// Kept records after which Git is stopped. Zero runs nothing.
    pub max_records: usize,
    /// Wall-clock budget for reading; spent, Git is killed and the call fails with
    /// [`GitError::TimedOut`], whatever it had collected.
    pub timeout: Duration,
    /// Exits besides success that are an answer, not a failure, as `grep` uses 1 for "no match".
    pub accepted_exits: &'a [Exit],
}

enum Reading {
    /// Git closed its output.
    Drained,
    /// `max_records` were kept before that.
    Full,
}

impl GitRunner {
    /// Runs Git and hands each record of its output to `take`, which returns what to keep of it
    /// or `None` to drop it. Git is killed and reaped as soon as `spec.max_records` records are
    /// kept, so its exit then says nothing. A trailing record without a terminator is dropped.
    pub async fn stream_records<I, S, T, F>(
        &self,
        args: I,
        spec: &RecordStream<'_>,
        mut take: F,
    ) -> Result<Vec<T>, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        F: FnMut(&[u8]) -> Option<T>,
    {
        if spec.max_records == 0 {
            return Ok(Vec::new());
        }
        let (mut child, command) = self.spawn(args)?;
        drop(child.stdin.take());
        let (Some(stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(GitError::Spawn(io::Error::other(
                "git started without pipes",
            )));
        };
        // Drained beside stdout: a chatty stderr would otherwise block Git on a full pipe.
        let stderr = tokio::spawn(async move {
            let mut text = Vec::new();
            let _ = stderr.read_to_end(&mut text).await;
            String::from_utf8_lossy(&text).into_owned()
        });
        let mut kept = Vec::new();
        let reading = timeout(
            spec.timeout,
            read_records(stdout, spec, &mut take, &mut kept),
        )
        .await;
        let drained = match reading {
            Ok(Ok(Reading::Drained)) => true,
            Ok(Ok(Reading::Full)) => false,
            Ok(Err(source)) => {
                stop(&mut child, &stderr).await;
                return Err(GitError::Stream { command, source });
            }
            Err(_elapsed) => {
                stop(&mut child, &stderr).await;
                return Err(GitError::TimedOut {
                    command,
                    after: spec.timeout,
                });
            }
        };
        if !drained {
            stop(&mut child, &stderr).await;
            return Ok(kept);
        }
        let status = child.wait().await.map_err(GitError::Spawn)?;
        let exit = Exit::from(status);
        if exit == Exit::Success || spec.accepted_exits.contains(&exit) {
            return Ok(kept);
        }
        Err(GitError::Failed {
            command,
            exit,
            stderr: stderr.await.unwrap_or_default().trim().to_owned(),
        })
    }
}

async fn stop(child: &mut Child, stderr: &JoinHandle<String>) {
    // Already gone is as good as killed.
    let _ = child.kill().await;
    stderr.abort();
}

async fn read_records<T, F>(
    mut stdout: ChildStdout,
    spec: &RecordStream<'_>,
    take: &mut F,
    kept: &mut Vec<T>,
) -> io::Result<Reading>
where
    F: FnMut(&[u8]) -> Option<T>,
{
    let mut chunk = vec![0_u8; READ_CHUNK];
    let (mut record, mut fields) = (Vec::new(), 0_usize);
    loop {
        let read = stdout.read(&mut chunk).await?;
        let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
            return Ok(Reading::Drained);
        };
        for &byte in bytes {
            if byte == NUL {
                fields += 1;
            }
            if byte != spec.terminator || fields < spec.leading_fields {
                record.push(byte);
                continue;
            }
            let item = take(&record);
            record.clear();
            fields = 0;
            if let Some(item) = item {
                kept.push(item);
                if kept.len() >= spec.max_records {
                    return Ok(Reading::Full);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "git_stream_tests.rs"]
mod tests;
