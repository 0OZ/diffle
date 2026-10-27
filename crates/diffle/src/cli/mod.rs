//! The `diffle` command line: parses arguments, then serves a review or edits the config.
mod args;
mod completion;
mod config;
mod lifetime;
mod lsp;
mod open;
mod paint;
mod parse;
pub mod repository;
mod serve;
mod timing;

use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches};

use crate::error::{Error, GitError};
use crate::github::pulls::repo_of_pr_url;
use crate::protocol::EnterRequest;
use args::{Cli, Command};

/// Runs the command line. Usage errors and user errors exit 2, anything else 1.
pub fn main() -> ExitCode {
    let matches = match Cli::command().try_get_matches() {
        Ok(matches) => matches,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { ExitCode::from(2) } else { ExitCode::SUCCESS };
        }
    };
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
    match runtime.block_on(dispatch(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            crate::status!("{} {e:#}", paint::red("✖"));
            ExitCode::from(if is_user_error(&e) { 2 } else { 1 })
        }
    }
}

async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let (req, later) = match cli.command {
        Some(Command::Config) => return config::run().await,
        Some(Command::Lsp) => return lsp::list().await,
        Some(Command::Completion { shell }) => {
            completion::print(shell);
            return Ok(());
        }
        Some(Command::Working { opts }) => (EnterRequest::Working, opts),
        Some(Command::Show { rev, opts }) => (EnterRequest::Revspec { args: vec![format!("{rev}^!")] }, opts),
        Some(Command::Pr { pr, opts }) => (EnterRequest::Pr { pr }, opts),
        Some(Command::RangeDiff { ranges, opts }) => (EnterRequest::RangeDiff { args: ranges }, opts),
        // No revisions: review the working tree, like a bare `git diff`.
        None if cli.revs.is_empty() => (EnterRequest::Working, Default::default()),
        // One pull-request URL is that pull request. A URL that only looks like one is an error,
        // not a revision. Anything else stays a revspec.
        None => match lone_pull_request(&cli.revs)? {
            Some(pr) => (EnterRequest::Pr { pr: Some(pr) }, Default::default()),
            None => (EnterRequest::Revspec { args: cli.revs }, Default::default()),
        },
    };
    serve::run(req, cli.global.merge(later)).await
}

/// The only positional argument, when it is an HTTP(S) pull-request URL on any host.
/// A selector that has that shape but does not parse is the parser's error, not a revision.
fn lone_pull_request(revs: &[String]) -> Result<Option<String>, Error> {
    let [only] = revs else { return Ok(None) };
    if !pull_request_url_shape(only) {
        return Ok(None);
    }
    repo_of_pr_url(only)?;
    Ok(Some(only.clone()))
}

/// `http(s)://…/pull/…`, whether or not the rest is a pull request.
fn pull_request_url_shape(selector: &str) -> bool {
    let lower = selector.to_ascii_lowercase();
    let http = lower.starts_with("https://") || lower.starts_with("http://");
    http && lower.contains("/pull/")
}

/// Whether the reviewer's input caused `e`: a revision, a Git or a GitHub error.
fn is_user_error(e: &anyhow::Error) -> bool {
    e.downcast_ref::<Error>().is_some_and(Error::is_user_error) || e.downcast_ref::<GitError>().is_some()
}
