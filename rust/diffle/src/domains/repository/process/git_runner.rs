//! Starts `git`. The only module in the crate that does.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

use super::git_error::GitError;

const PROGRAM: &str = "git";
/// `diff`, `log` and `grep` are porcelain and honour the user's config. These overrides keep
/// their output in the shape the parsers expect: no colour, `a/` `b/` prefixes, and non-ASCII
/// paths written verbatim instead of C-quoted. `color.ui` does not beat a per-command
/// `color.diff` or `color.grep` set to `always`, so those are pinned too.
const PINNED_CONFIG: [&str; 12] = [
    "-c",
    "color.ui=never",
    "-c",
    "color.diff=never",
    "-c",
    "color.grep=never",
    "-c",
    "diff.noprefix=false",
    "-c",
    "diff.mnemonicPrefix=false",
    "-c",
    "core.quotePath=false",
];
/// Reading must not take the index lock: the user's own Git commands run beside us.
const OPTIONAL_LOCKS: (&str, &str) = ("GIT_OPTIONAL_LOCKS", "0");

/// How a Git process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Success,
    Failure(i32),
    /// Ended by a signal, so it has no exit code.
    Terminated,
}

impl Exit {
    /// Git answers "no such thing" (`merge-base`, `rev-parse --verify`, `config --get`) with 1.
    pub const NO_ANSWER: Self = Self::Failure(1);
}

impl From<ExitStatus> for Exit {
    fn from(status: ExitStatus) -> Self {
        match (status.success(), status.code()) {
            (true, _) => Self::Success,
            (false, Some(code)) => Self::Failure(code),
            (false, None) => Self::Terminated,
        }
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => f.write_str("exit 0"),
            Self::Failure(code) => write!(f, "exit {code}"),
            Self::Terminated => f.write_str("killed by a signal"),
        }
    }
}

/// What a successful Git command printed.
#[derive(Debug)]
pub struct Stdout(Vec<u8>);

impl Stdout {
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }

    pub fn trimmed(&self) -> String {
        self.text().trim().to_owned()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// The NUL-terminated records of a `-z` listing; an empty output has none.
    pub fn nul_records(&self) -> Vec<String> {
        let text = self.text();
        let records = text.strip_suffix('\0').unwrap_or(&text);
        match records.is_empty() {
            true => Vec::new(),
            false => records.split('\0').map(str::to_owned).collect(),
        }
    }
}

/// A Git process that ran to its end. A non-zero exit is data here, not yet an error: Git
/// uses exit 1 to answer "no" (`merge-base`, `rev-parse --verify`).
#[derive(Debug)]
#[must_use = "an unchecked exit hides a failed git command"]
pub struct Finished {
    exit: Exit,
    stdout: Vec<u8>,
    stderr: String,
    command: String,
}

impl Finished {
    pub fn exit(&self) -> Exit {
        self.exit
    }

    /// The output of a command that must have succeeded.
    pub fn into_stdout(self) -> Result<Stdout, GitError> {
        self.into_stdout_allowing(&[])
    }

    /// The output of a command that succeeded or ended in one of the `accepted` exits, as
    /// `diff --no-index` does with 1 for "the files differ".
    pub fn into_stdout_allowing(self, accepted: &[Exit]) -> Result<Stdout, GitError> {
        match (self.exit, accepted.contains(&self.exit)) {
            (Exit::Success, _) | (_, true) => Ok(Stdout(self.stdout)),
            (Exit::Failure(_) | Exit::Terminated, false) => Err(GitError::Failed {
                command: self.command,
                exit: self.exit,
                stderr: self.stderr.trim().to_owned(),
            }),
        }
    }
}

/// Runs Git in one directory with explicit argv, never through a shell.
#[derive(Debug, Clone)]
pub struct GitRunner {
    cwd: PathBuf,
}

impl GitRunner {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self { cwd: cwd.into() }
    }

    pub async fn finish<I, S>(&self, args: I) -> Result<Finished, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.finish_with_input(args, &[]).await
    }

    /// Like [`finish`](Self::finish), with `input` on stdin (a path list, a tree to hash).
    pub async fn finish_with_input<I, S>(&self, args: I, input: &[u8]) -> Result<Finished, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let (mut child, command) = self.spawn(args)?;
        let stdin = child.stdin.take();
        // Feeding and draining run together: a large input would otherwise deadlock on a full pipe.
        let feed = async move {
            let Some(mut stdin) = stdin else { return };
            // A command that exits before reading everything (an early failure) closes the pipe; its exit says why.
            let _ = stdin.write_all(input).await;
            let _ = stdin.shutdown().await;
        };
        let (output, ()) = tokio::join!(child.wait_with_output(), feed);
        let output = output.map_err(GitError::Spawn)?;
        Ok(Finished {
            exit: output.status.into(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            command,
        })
    }

    /// Starts Git with the pinned config and all three pipes open; the child dies with its
    /// handle. Also returns the command line, for error messages.
    pub(super) fn spawn<I, S>(&self, args: I) -> Result<(Child, String), GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<S> = args.into_iter().collect();
        let command = args
            .iter()
            .map(|arg| arg.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        let child = Command::new(PROGRAM)
            .args(PINNED_CONFIG)
            .args(&args)
            .current_dir(&self.cwd)
            .env(OPTIONAL_LOCKS.0, OPTIONAL_LOCKS.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(GitError::Spawn)?;
        Ok((child, command))
    }

    /// The output of a command that must succeed.
    pub async fn stdout<I, S>(&self, args: I) -> Result<Stdout, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.finish(args).await?.into_stdout()
    }
}
