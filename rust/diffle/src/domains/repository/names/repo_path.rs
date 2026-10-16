//! A path inside the repository, as Git names it: relative, `/`-separated, never leaving the root.

use std::fmt;
use std::path::{Component, Path};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RepoPathError {
    #[error("a repository path is not empty")]
    Empty,
    #[error("a repository path is relative to the root")]
    Absolute,
    #[error("a repository path stays inside the root")]
    Traversal,
    #[error("a repository path names a file, not a directory")]
    Directory,
    #[error("a repository path has no NUL")]
    Nul,
}

impl RepoPath {
    pub fn parse(text: &str) -> Result<Self, RepoPathError> {
        if text.is_empty() {
            return Err(RepoPathError::Empty);
        }
        if text.contains('\0') {
            return Err(RepoPathError::Nul);
        }
        if text.ends_with('/') {
            return Err(RepoPathError::Directory);
        }
        for component in Path::new(text).components() {
            match component {
                Component::ParentDir => return Err(RepoPathError::Traversal),
                Component::RootDir | Component::Prefix(_) => return Err(RepoPathError::Absolute),
                Component::CurDir | Component::Normal(_) => {}
            }
        }
        Ok(Self(text.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nested_and_unusual_names() {
        for text in [
            "a.txt",
            "src/lib.rs",
            "dir/with space/ü.txt",
            ".github/ci.yml",
            "a\nb",
        ] {
            assert_eq!(RepoPath::parse(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn rejects_what_would_leave_the_root_or_name_no_file() {
        assert_eq!(RepoPath::parse(""), Err(RepoPathError::Empty));
        assert_eq!(RepoPath::parse("../x"), Err(RepoPathError::Traversal));
        assert_eq!(RepoPath::parse("a/../../x"), Err(RepoPathError::Traversal));
        assert_eq!(RepoPath::parse("/etc/passwd"), Err(RepoPathError::Absolute));
        assert_eq!(RepoPath::parse("dir/"), Err(RepoPathError::Directory));
        assert_eq!(RepoPath::parse("a\0b"), Err(RepoPathError::Nul));
    }

    #[test]
    fn orders_like_the_strings_git_prints() {
        let mut paths: Vec<_> = ["b", "a/b", "a.txt", "a"]
            .into_iter()
            .map(|text| RepoPath::parse(text).unwrap())
            .collect();
        paths.sort();
        let sorted: Vec<_> = paths.iter().map(RepoPath::as_str).collect();
        assert_eq!(sorted, ["a", "a.txt", "a/b", "b"]);
    }
}
