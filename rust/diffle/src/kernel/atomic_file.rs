//! Writes a file so a reader sees the old text or the new, never half of one.

use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::fs;

#[cfg(unix)]
const DIR_MODE: u32 = 0o700;
#[cfg(unix)]
pub const PRIVATE_FILE_MODE: u32 = 0o600;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Writes `text` to `file`: a uniquely named temp file beside it, then a rename. The directory
/// is created owner-only; a private file is readable by its owner alone. A failed write leaves
/// no temp file behind.
pub async fn write_file_atomic(file: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        create_private_dir(dir).await?;
    }
    let temp = temp_name(file);
    let written = write_private(&temp, text).await;
    let renamed = match written {
        Ok(()) => fs::rename(&temp, file).await,
        Err(error) => Err(error),
    };
    if renamed.is_err() {
        // The write error is the one worth reporting; a temp file already gone is fine.
        let _ = fs::remove_file(&temp).await;
    }
    renamed
}

/// Creates `dir` and its parents, owner-only where the platform has modes.
pub async fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(DIR_MODE);
    builder.create(dir).await
}

// pid alone collides between instances in one process; the counter makes the name unique.
fn temp_name(file: &Path) -> PathBuf {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = uuid::Uuid::now_v7().simple();
    let mut name = file.as_os_str().to_owned();
    name.push(format!(".{}.{sequence}.{nonce}.tmp", process::id()));
    PathBuf::from(name)
}

async fn write_private(path: &Path, text: &str) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(PRIVATE_FILE_MODE);
    let mut file = options.open(path).await?;
    file.write_all(text.as_bytes()).await?;
    file.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replaces_the_text_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested/state.json");
        write_file_atomic(&file, "one").await.unwrap();
        write_file_atomic(&file, "two").await.unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        let left: Vec<_> = std::fs::read_dir(file.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(left, [std::ffi::OsString::from("state.json")]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_file_and_its_new_directory_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("private/state.json");
        write_file_atomic(&file, "x").await.unwrap();
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), PRIVATE_FILE_MODE);
        assert_eq!(mode(file.parent().unwrap()), DIR_MODE);
    }

    #[tokio::test]
    async fn a_failed_rename_cleans_up_the_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("taken");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("child"), "x").unwrap();
        assert!(write_file_atomic(&target, "x").await.is_err());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("taken")]);
    }
}
