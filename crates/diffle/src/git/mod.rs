//! All Git execution. Calls run with cwd = the repository root, except reading a submodule's HEAD.
mod catfile;
mod changes;
mod checkout;
mod commits;
pub mod exec;
mod files;
mod grep;
mod patch;
mod refs;

use std::path::{Path, PathBuf};

use catfile::CatFileBatch;
pub use catfile::CatFileObject;
pub use changes::is_binary;
pub use grep::{GrepMatch, GrepOptions, GrepResult};
pub use refs::{Remote, Upstream};

use crate::error::{GitError, Result};
use exec::{ExecOptions, GitEnvironment, exec_git, text};

/// Git's `core.bigFileThreshold` default: a file this large is diffed as binary without reading
/// it, which also bounds what sizing an untracked file loads.
pub const BIG_FILE_THRESHOLD: u64 = 512 * 1024 * 1024;
/// Concurrent worktree reads while sizing or hashing files.
const READ_CONCURRENCY: usize = 8;

/// The uncommitted tree, accepted on either side of a comparison.
pub const WORKTREE: &str = "worktree";

pub struct GitRepo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    /// Shared dir for worktrees (refs, packed-refs). Equals `git_dir` for the main worktree.
    pub common_dir: PathBuf,
    /// Where this run fetches pull requests; unique, so two servers never share refs.
    pub review_refs: String,
    cat_file: CatFileBatch,
    /// `open` is ordinary. Only `clone_into` may be checked out.
    isolated: bool,
    environment: GitEnvironment,
    /// Pseudo-git-root whose `diffle` child holds review comments. Defaults to `git_dir`.
    review_state_dir: PathBuf,
}

impl GitRepo {
    pub async fn open(dir: &Path) -> Result<Self> {
        Self::open_with_environment(dir, GitEnvironment::Inherited).await
    }

    async fn open_with_environment(dir: &Path, environment: GitEnvironment) -> Result<Self> {
        let out = exec_git(
            dir,
            &["rev-parse", "--show-toplevel", "--absolute-git-dir", "--git-common-dir"],
            ExecOptions { environment, ..Default::default() },
        )
        .await?;
        let out = text(out);
        let mut lines = out.trim().lines();
        let (Some(root), Some(git_dir)) = (lines.next(), lines.next()) else {
            return Err(GitError {
                code: Some(128),
                ..GitError::refused(format!("not a git repository: {}", dir.display()), &[])
            }
            .into());
        };
        // Git prints POSIX separators even on Windows; normalise them to native ones.
        let root = native(root);
        let git_dir = native(git_dir);
        let common_dir = lines.next().map_or_else(|| git_dir.clone(), |common| native_path(&root.join(native(common))));
        let review_state_dir = git_dir.clone();
        Ok(Self {
            cat_file: CatFileBatch::new(root.clone(), environment),
            review_refs: format!("refs/diffle/{}", uuid::Uuid::new_v4()),
            root,
            git_dir,
            common_dir,
            isolated: false,
            environment,
            review_state_dir,
        })
    }

    /// Clone into an empty directory; full history preserves merge bases. The result is an owned
    /// checkout: `checkout_owned` may detach it, and `open` may not.
    pub async fn clone_into(url: &str, dir: &Path) -> Result<Self> {
        let environment = GitEnvironment::Isolated;
        let opts = ExecOptions { environment, env: &[("GIT_TERMINAL_PROMPT", "0")], ..Default::default() };
        exec_git(dir, &["clone", "--quiet", "--no-tags", "--", url, "."], opts).await?;
        let mut repo = Self::open_with_environment(dir, environment).await?;
        let (root, git_dir, common_dir) = owned_paths(dir, &repo.root, &repo.git_dir, &repo.common_dir)?;
        repo.cat_file = CatFileBatch::new(root.clone(), environment);
        repo.review_state_dir = git_dir.clone();
        repo.root = root;
        repo.git_dir = git_dir;
        repo.common_dir = common_dir;
        repo.isolated = true;
        Ok(repo)
    }

    /// How many `cat-file --batch` processes this repository has started. Diagnostics and tests.
    pub fn cat_file_spawns(&self) -> usize {
        self.cat_file.spawns()
    }

    async fn exec(&self, args: &[&str], opts: ExecOptions<'_>) -> Result<Vec<u8>, GitError> {
        exec_git(&self.root, args, ExecOptions { environment: self.environment, ..opts }).await
    }

    async fn text(&self, args: &[&str]) -> Result<String, GitError> {
        Ok(text(self.exec(args, ExecOptions::default()).await?))
    }

    async fn text_with(&self, args: &[&str], opts: ExecOptions<'_>) -> Result<String, GitError> {
        Ok(text(self.exec(args, opts).await?))
    }
}

/// Ownership is a filesystem fact, not just a successful `open` or a lexical path prefix.
fn owned_paths(dir: &Path, root: &Path, git_dir: &Path, common_dir: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let canonical = |path: &Path| {
        dunce::canonicalize(path).map_err(|error| {
            GitError::refused(format!("cannot validate owned clone path {}: {error}", path.display()), &["clone"])
        })
    };
    let dir = canonical(dir)?;
    let root = canonical(root)?;
    let git_dir = canonical(git_dir)?;
    let common_dir = canonical(common_dir)?;
    if root != dir || !git_dir.starts_with(&dir) || !common_dir.starts_with(&dir) {
        return Err(GitError::refused("refusing owned clone with repository paths outside its root", &["clone"]).into());
    }
    Ok((root, git_dir, common_dir))
}

fn native(path: &str) -> PathBuf {
    native_path(Path::new(path))
}

/// Separators as the platform writes them, and `.`/`..` folded, so paths compare equal to joined ones.
fn native_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Git reverses the commit-to-worktree diff when the worktree is the old endpoint.
fn diff_range<'a>(old: &'a str, new: &'a str) -> Vec<&'a str> {
    if old == WORKTREE {
        vec!["-R", new]
    } else if new == WORKTREE {
        vec![old]
    } else {
        vec![old, new]
    }
}

/// `path` under `root`, or none when it would leave the repository.
fn inside(root: &Path, path: &str) -> Option<PathBuf> {
    let joined = native_path(&root.join(path));
    joined.starts_with(root).then_some(joined)
}
