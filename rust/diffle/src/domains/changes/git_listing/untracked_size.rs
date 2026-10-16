//! Sizing an untracked file for the changed-file list without loading it whole.

use std::path::Path;

use tokio::fs;
use tokio::io::AsyncReadExt;

use crate::domains::changes::changed_file::Delta;
use crate::domains::repository::contents::binary::{BINARY_SNIFF_BYTES, is_binary};

/// A file this large is diffed as binary without being read, as Git's own default does.
pub const BIG_FILE_BYTES: u64 = 512 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;

/// The lines `file` would add (or remove, seen from the other side) if it were diffed against
/// nothing. A symlink counts its target text, as Git stores it. A file that cannot be read
/// counts as empty: one bad path must not fail the snapshot.
pub async fn untracked_delta(file: &Path, reversed: bool) -> Delta {
    let lines = match measure(file).await {
        Ok(Measured::Binary) => return Delta::Binary,
        Ok(Measured::Lines(lines)) => lines,
        Err(_) => 0,
    };
    match reversed {
        true => Delta::Text {
            additions: 0,
            deletions: lines,
        },
        false => Delta::Text {
            additions: lines,
            deletions: 0,
        },
    }
}

enum Measured {
    Binary,
    Lines(u32),
}

async fn measure(file: &Path) -> std::io::Result<Measured> {
    let metadata = fs::symlink_metadata(file).await?;
    if metadata.is_symlink() {
        let target = fs::read_link(file).await?;
        return Ok(count(target.as_os_str().as_encoded_bytes()));
    }
    if !metadata.is_file() {
        return Ok(Measured::Lines(0));
    }
    if metadata.len() >= BIG_FILE_BYTES {
        return Ok(Measured::Binary);
    }
    let mut reader = fs::File::open(file).await?;
    let (mut chunk, mut seen, mut newlines) = (vec![0_u8; CHUNK], 0_usize, 0_u32);
    let mut last = None;
    loop {
        let read = reader.read(&mut chunk).await?;
        let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
            break;
        };
        let sniffed = BINARY_SNIFF_BYTES.saturating_sub(seen).min(bytes.len());
        if bytes.get(..sniffed).is_some_and(is_binary) {
            return Ok(Measured::Binary);
        }
        seen += bytes.len();
        newlines +=
            u32::try_from(bytes.iter().filter(|&&byte| byte == b'\n').count()).unwrap_or(u32::MAX);
        last = bytes.last().copied();
    }
    Ok(Measured::Lines(match last {
        None | Some(b'\n') => newlines,
        Some(_) => newlines + 1,
    }))
}

fn count(bytes: &[u8]) -> Measured {
    if is_binary(bytes) {
        return Measured::Binary;
    }
    let newlines =
        u32::try_from(bytes.iter().filter(|&&byte| byte == b'\n').count()).unwrap_or(u32::MAX);
    Measured::Lines(match bytes.last() {
        None | Some(b'\n') => newlines,
        Some(_) => newlines + 1,
    })
}

#[cfg(test)]
#[path = "untracked_size_tests.rs"]
mod tests;
