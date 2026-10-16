//! Mutual exclusion on one file, across the processes on this machine and the tasks in this one.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, SystemTime};

use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex as AsyncMutex;

use super::atomic_file::create_private_dir;
use super::epoch_millis::EpochMillis;

/// A holder older than this is presumed hung, even when its pid is alive.
const LOCK_STALE_AFTER: Duration = Duration::from_secs(30);
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
const RETRY_BASE_MS: u64 = 10;
const RETRY_JITTER_MS: u64 = 20;
const LOCK_SUFFIX: &str = ".lock";

#[derive(Debug, Error)]
pub enum LockError {
    #[error("timed out waiting for {}", .0.display())]
    TimedOut(PathBuf),
    #[error("cannot lock {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Runs `work` while holding `<file>.lock`. A lock whose pid is dead, or older than 30
/// seconds, is taken over; waiting longer than 10 seconds fails. The lock is released
/// whether `work` succeeds or not.
pub async fn with_file_lock<T, Fut>(file: &Path, work: impl FnOnce() -> Fut) -> Result<T, LockError>
where
    Fut: Future<Output = T>,
{
    // Queue in-process first so tasks of one server never spin on the lock file.
    let queue = in_process_queue(file);
    let _turn = queue.lock().await;
    let lock = lock_path(file);
    acquire(&lock, LOCK_TIMEOUT).await?;
    let result = work().await;
    // A lock that cannot be removed goes stale by age; the work's result still stands.
    let _ = fs::remove_file(&lock).await;
    Ok(result)
}

fn lock_path(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(LOCK_SUFFIX);
    PathBuf::from(name)
}

fn in_process_queue(file: &Path) -> Arc<AsyncMutex<()>> {
    static QUEUES: OnceLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let mut queues = QUEUES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    Arc::clone(queues.entry(file.to_owned()).or_default())
}

async fn acquire(lock: &Path, timeout: Duration) -> Result<(), LockError> {
    let io_error = |source| LockError::Io {
        path: lock.to_owned(),
        source,
    };
    if let Some(dir) = lock.parent() {
        create_private_dir(dir).await.map_err(io_error)?;
    }
    let deadline = EpochMillis::now().get() + u64::try_from(timeout.as_millis()).unwrap_or(0);
    loop {
        match try_create(lock).await {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(error)),
        }
        if is_stale(lock).await.map_err(io_error)? {
            // Another waiter may take it over first; either way the next attempt decides.
            let _ = fs::remove_file(lock).await;
            continue;
        }
        if EpochMillis::now().get() > deadline {
            return Err(LockError::TimedOut(lock.to_owned()));
        }
        tokio::time::sleep(retry_delay()).await;
    }
}

async fn try_create(lock: &Path) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(super::atomic_file::PRIVATE_FILE_MODE);
    let mut file = options.open(lock).await?;
    file.write_all(format!("{}\n", process::id()).as_bytes())
        .await?;
    file.flush().await
}

fn retry_delay() -> Duration {
    // Spread contenders without a random source: the clock's low digits are jitter enough.
    let jitter = EpochMillis::now().get() % RETRY_JITTER_MS;
    Duration::from_millis(RETRY_BASE_MS + jitter)
}

/// True when the holder is gone: a dead pid, or too old to trust. A lock with no pid yet is
/// being taken, and one that vanished is not stale.
async fn is_stale(lock: &Path) -> io::Result<bool> {
    let (metadata, text) = match tokio::try_join!(fs::metadata(lock), fs::read_to_string(lock)) {
        Ok(found) => found,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let age = metadata
        .modified()
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .unwrap_or_default();
    if age > LOCK_STALE_AFTER {
        return Ok(true);
    }
    Ok(text
        .trim()
        .parse::<u32>()
        .is_ok_and(|pid| pid > 0 && holder_is_dead(pid)))
}

#[cfg(unix)]
fn holder_is_dead(pid: u32) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    test_kill_process(pid) == Err(Errno::SRCH)
}

/// Without a way to ask, a holder is presumed alive until its lock ages out.
#[cfg(not(unix))]
fn holder_is_dead(_pid: u32) -> bool {
    false
}

#[cfg(test)]
#[path = "file_lock_tests.rs"]
mod tests;
