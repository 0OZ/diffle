//! Which paths exist: the index, a commit's tree, untracked files, and Git's own verdicts on
//! ignore rules and attributes.

use std::collections::{HashMap, HashSet};

use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::names::tree_source::TreeSource;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::Exit;

/// Path arguments are names, never patterns: `src/[id].tsx` is that file, not a character class.
pub(in crate::domains::repository) const LITERAL_PATHSPECS: &str = "--literal-pathspecs";

/// A gitattribute's boolean value on one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrValue {
    Set,
    Unset,
}

impl GitRepo {
    /// Every path in the index.
    pub async fn index_paths(&self) -> Result<Vec<String>, GitError> {
        Ok(self.runner.stdout(["ls-files", "-z"]).await?.nul_records())
    }

    /// Every path in a commit's tree.
    pub async fn tree_paths(&self, tree: &ObjectId) -> Result<Vec<String>, GitError> {
        Ok(self
            .runner
            .stdout(["ls-tree", "-r", "-z", "--name-only", tree.as_str()])
            .await?
            .nul_records())
    }

    /// Untracked, not ignored. A nested repository is listed as `dir/`.
    pub async fn untracked_paths(&self) -> Result<Vec<String>, GitError> {
        Ok(self
            .runner
            .stdout(["ls-files", "-z", "--others", "--exclude-standard"])
            .await?
            .nul_records())
    }

    /// The ignored entries, collapsed to a directory where the whole directory is ignored.
    pub async fn ignored_entries(&self) -> Result<Vec<String>, GitError> {
        Ok(self
            .runner
            .stdout([
                "ls-files",
                "-z",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
            ])
            .await?
            .nul_records())
    }

    /// Whether the index knows `path`.
    pub async fn is_tracked(&self, path: &RepoPath) -> Result<bool, GitError> {
        let finished = self
            .runner
            .finish([
                LITERAL_PATHSPECS,
                "ls-files",
                "-z",
                "--error-unmatch",
                "--",
                path.as_str(),
            ])
            .await?;
        Ok(match finished.exit() {
            Exit::Success => true,
            Exit::Failure(_) | Exit::Terminated => false,
        })
    }

    /// Which of `paths` Git ignores.
    pub async fn ignored_among(&self, paths: &[String]) -> Result<HashSet<String>, GitError> {
        if paths.is_empty() {
            return Ok(HashSet::new());
        }
        let finished = self
            .runner
            .finish_with_input(
                ["check-ignore", "-z", "--stdin"],
                paths.join("\0").as_bytes(),
            )
            .await?;
        let stdout = finished.into_stdout_allowing(&[Exit::NO_ANSWER])?;
        Ok(stdout.nul_records().into_iter().collect())
    }

    /// The boolean gitattribute `name` per path: `attr` and `attr=true` are set, `-attr` and
    /// `attr=false` unset, and a path with neither is absent. A commit reads `.gitattributes`
    /// from its tree, the worktree from the checkout.
    pub async fn bool_attribute(
        &self,
        name: &str,
        paths: &[&RepoPath],
        source: &TreeSource,
    ) -> Result<HashMap<String, AttrValue>, GitError> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }
        let mut args = vec![
            "check-attr".to_owned(),
            "-z".to_owned(),
            "--stdin".to_owned(),
        ];
        if let TreeSource::Object(tree) = source {
            args.push(format!("--source={tree}"));
        }
        args.extend([name.to_owned(), "--".to_owned()]);
        let input = paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join("\0");
        let fields = self
            .runner
            .finish_with_input(args, input.as_bytes())
            .await?
            .into_stdout()?
            .nul_records();
        // Records are `path NUL attribute NUL value NUL`.
        Ok(fields
            .chunks_exact(3)
            .filter_map(|record| match record {
                [path, _, value] => attr_value(value).map(|value| (path.clone(), value)),
                _ => None,
            })
            .collect())
    }
}

fn attr_value(text: &str) -> Option<AttrValue> {
    match text {
        "set" | "true" => Some(AttrValue::Set),
        "unset" | "false" => Some(AttrValue::Unset),
        _ => None,
    }
}

#[cfg(test)]
#[path = "listings_tests.rs"]
mod tests;
