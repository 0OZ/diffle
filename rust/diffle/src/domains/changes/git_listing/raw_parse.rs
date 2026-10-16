//! Reading `diff -z -M --raw --no-abbrev --numstat`.
//!
//! Raw records come first: `:srcmode dstmode srcsha dstsha status`, then the path (a rename or
//! copy gives the source and then the destination). Numstat records follow: `add\tdel\tpath`,
//! with `-` counts for binary files; a rename writes `add\tdel\t` and then both paths as their
//! own records. Each side's blob is only known against a commit: the worktree's is zeros.
//! A gitlink has mode 160000 on a side.

use std::collections::HashMap;
use std::iter::Peekable;
use std::slice::Iter;

use crate::domains::changes::change_kind::ChangeKind;
use crate::domains::changes::changed_file::{ChangedFile, Delta};
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;

const GITLINK_MODE: &str = "160000";
const RAW_MARKER: char = ':';
const BINARY_COUNT: &str = "-";

pub fn parse_raw_numstat(records: &[String]) -> Result<Vec<ChangedFile>, GitError> {
    let mut records = records.iter().peekable();
    let mut files = Vec::new();
    let mut by_path = HashMap::new();
    while let Some(raw) = records.next_if(|record| record.starts_with(RAW_MARKER)) {
        let file = raw_file(raw, &mut records)?;
        by_path.insert(file.path.to_string(), files.len());
        files.push(file);
    }
    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(inline)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(unexpected("a numstat record", record));
        };
        let path = match inline.is_empty() {
            false => inline,
            true => {
                let _source = records.next();
                records
                    .next()
                    .ok_or_else(|| unexpected("a renamed path", record))?
            }
        };
        let Some(&at) = by_path.get(path) else {
            continue;
        };
        if let Some(file) = files.get_mut(at) {
            file.delta = delta(added, deleted, record)?;
        }
    }
    Ok(files)
}

fn raw_file(raw: &str, records: &mut Peekable<Iter<'_, String>>) -> Result<ChangedFile, GitError> {
    let mut fields = raw.split(' ');
    let (Some(src_mode), Some(dst_mode), Some(src_sha), Some(dst_sha), Some(status)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(unexpected("a raw diff record", raw));
    };
    let from = match status.starts_with(['R', 'C']) {
        true => Some(path(records.next(), raw)?),
        false => None,
    };
    let new_path = path(records.next(), raw)?;
    let kind = ChangeKind::parse(status, from).ok_or_else(|| unexpected("a change status", raw))?;
    let src_mode = src_mode.trim_start_matches(RAW_MARKER);
    let submodule = dst_mode == GITLINK_MODE || (kind.is_deleted() && src_mode == GITLINK_MODE);
    Ok(ChangedFile {
        path: new_path,
        kind,
        delta: Delta::Text {
            additions: 0,
            deletions: 0,
        },
        blob: blob(dst_sha, raw)?,
        old_blob: blob(src_sha, raw)?,
        generated: false,
        submodule,
    })
}

fn path(record: Option<&String>, context: &str) -> Result<RepoPath, GitError> {
    let text = record.ok_or_else(|| unexpected("a path", context))?;
    Ok(RepoPath::parse(text)?)
}

/// Zeros mean the side has no blob git could name.
fn blob(sha: &str, context: &str) -> Result<Option<ObjectId>, GitError> {
    match sha.bytes().all(|byte| byte == b'0') {
        true => Ok(None),
        false => ObjectId::parse(sha)
            .map(Some)
            .map_err(|_| unexpected("an object id", context)),
    }
}

fn delta(added: &str, deleted: &str, record: &str) -> Result<Delta, GitError> {
    if added == BINARY_COUNT {
        return Ok(Delta::Binary);
    }
    match (added.parse(), deleted.parse()) {
        (Ok(additions), Ok(deletions)) => Ok(Delta::Text {
            additions,
            deletions,
        }),
        (Err(_), _) | (_, Err(_)) => Err(unexpected("line counts", record)),
    }
}

fn unexpected(expected: &'static str, output: &str) -> GitError {
    GitError::Unexpected {
        expected,
        output: output.to_owned(),
    }
}

#[cfg(test)]
#[path = "raw_parse_tests.rs"]
mod tests;
