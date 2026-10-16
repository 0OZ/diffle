//! `git diff --raw --numstat`: which files changed, and how much.

use super::diff_sides::DiffSides;
use crate::domains::repository::contents::listings::LITERAL_PATHSPECS;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::repository::process::git_error::GitError;

impl GitRepo {
    /// The NUL-terminated records of `diff -z -M --raw --no-abbrev --numstat` between the
    /// sides: the raw records first, then the numstat ones. Empty for [`DiffSides::Nothing`].
    pub async fn raw_numstat(&self, sides: &DiffSides) -> Result<Vec<String>, GitError> {
        if *sides == DiffSides::Nothing {
            return Ok(Vec::new());
        }
        let mut args = vec![
            LITERAL_PATHSPECS.to_owned(),
            "diff".to_owned(),
            "-z".to_owned(),
            "-M".to_owned(),
            "--raw".to_owned(),
            "--no-abbrev".to_owned(),
            "--numstat".to_owned(),
        ];
        args.extend(sides.range_args());
        Ok(self.runner.stdout(args).await?.nul_records())
    }
}
