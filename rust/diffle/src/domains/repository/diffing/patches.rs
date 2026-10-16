//! Unified diffs as text.

use super::diff_sides::DiffSides;
use crate::domains::repository::contents::listings::LITERAL_PATHSPECS;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::repository::process::git_runner::Exit;

/// Git's `diff --no-index` exits 1 when the files differ.
const DIFFERS: Exit = Exit::NO_ANSWER;
const DEV_NULL: &str = "/dev/null";

impl GitRepo {
    /// The patch for `paths` between the sides, renames detected; every changed file when
    /// `paths` is empty. Name both sides of a rename, or it reads as an add and a delete.
    pub async fn patch_text(
        &self,
        sides: &DiffSides,
        context: u32,
        paths: &[&RepoPath],
    ) -> Result<String, GitError> {
        if *sides == DiffSides::Nothing {
            return Ok(String::new());
        }
        let mut args = vec![
            LITERAL_PATHSPECS.to_owned(),
            "diff".to_owned(),
            "-M".to_owned(),
        ];
        args.extend(sides.patch_args().map(str::to_owned));
        args.push(format!("-U{context}"));
        args.extend(sides.range_args());
        if !paths.is_empty() {
            args.push("--".to_owned());
            args.extend(paths.iter().map(|path| path.to_string()));
        }
        Ok(self.runner.stdout(args).await?.text().into_owned())
    }

    /// The patch of a worktree file the index does not know, against nothing: an addition, or
    /// (when the worktree is the old side) a deletion. Its header is rewritten to the
    /// conventional `a/<path> b/<path>` so parsers see an ordinary added or deleted file.
    pub async fn untracked_patch_text(
        &self,
        sides: &DiffSides,
        context: u32,
        path: &RepoPath,
    ) -> Result<String, GitError> {
        let reversed = sides.is_reversed();
        let mut args = vec!["diff".to_owned(), "--no-index".to_owned()];
        if reversed {
            args.push("-R".to_owned());
        }
        args.extend(sides.patch_args().map(str::to_owned));
        args.extend([
            format!("-U{context}"),
            "--".to_owned(),
            DEV_NULL.to_owned(),
            path.to_string(),
        ]);
        let text = self
            .runner
            .finish(args)
            .await?
            .into_stdout_allowing(&[DIFFERS])?
            .text()
            .into_owned();
        Ok(conventional_header(&text, reversed))
    }
}

/// Git names the missing side `dev/null` in the header; give it the file's own name.
fn conventional_header(patch: &str, reversed: bool) -> String {
    let mut rewritten = String::with_capacity(patch.len());
    let mut done = false;
    for line in patch.split_inclusive('\n') {
        let (text, ending) = split_ending(line);
        let replacement = match done {
            true => None,
            false => header_naming(text, reversed),
        };
        done |= replacement.is_some();
        rewritten.push_str(replacement.as_deref().unwrap_or(text));
        rewritten.push_str(ending);
    }
    rewritten
}

fn split_ending(line: &str) -> (&str, &str) {
    line.strip_suffix('\n')
        .map_or((line, ""), |text| (text, "\n"))
}

/// `diff --git a/dev/null "b/<path>"` or, reversed, `diff --git "a/<path>" b/dev/null`, each
/// side quoted on its own: the result repeats the quoting of the side that carries the path.
fn header_naming(line: &str, reversed: bool) -> Option<String> {
    let sides = line.strip_prefix("diff --git ")?;
    match reversed {
        false => {
            let rest = sides
                .strip_prefix('"')
                .unwrap_or(sides)
                .strip_prefix("a/dev/null")?;
            let rest = rest.strip_prefix('"').unwrap_or(rest).strip_prefix(' ')?;
            let (quote, named) = split_quote(rest);
            let path = named.strip_prefix("b/")?;
            Some(format!("diff --git {quote}a/{path} {quote}b/{path}"))
        }
        true => {
            let (quote, named) = split_quote(sides);
            let named = named.strip_prefix("a/")?;
            let head = named
                .strip_suffix("b/dev/null\"")
                .or_else(|| named.strip_suffix("b/dev/null"))?;
            let path = head
                .strip_suffix(" \"")
                .or_else(|| head.strip_suffix(' '))?;
            Some(format!("diff --git {quote}a/{path} {quote}b/{path}"))
        }
    }
}

fn split_quote(text: &str) -> (&str, &str) {
    text.strip_prefix('"')
        .map_or(("", text), |rest| ("\"", rest))
}

#[cfg(test)]
#[path = "patches_tests.rs"]
mod tests;
