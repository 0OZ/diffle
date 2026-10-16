//! The command line, as `clap` reads it. The only module that knows flag spellings.

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;
use crate::kernel::allowed_origin::AllowedOrigin;

const DEFAULT_HOST: &str = "127.0.0.1";

#[derive(Debug, Parser)]
#[command(
    name = "diffle",
    version,
    about = "Review a git diff in the browser and export line comments as an agent prompt.",
    disable_version_flag = true,
    after_help = "Shorthands (in place of <revs>):\n  \
        working        same as HEAD..worktree: uncommitted changes, staged and untracked included\n  \
        show [rev]     same as <rev>^!: one commit against its first parent (default: HEAD)\n\n\
        Revisions follow git diff, except that a lone one compares from the merge base:\n  \
        diffle main              same as main...HEAD\n  \
        diffle HEAD~3            the last three commits\n  \
        diffle main..feat        main vs feat\n  \
        diffle main...feat       what feat added since it left main\n  \
        diffle main..worktree    main vs the uncommitted tree\n  \
        diffle a1b2c3d^!         the commit a1b2c3d alone, like git show\n\n\
        Without revisions, diffle compares HEAD..worktree.\n\n\
        Status goes to stderr, so stdout carries only the review: unless --keep-alive is set,\n\
        closing the last auto-opened browser tab stops diffle and prints the open comments as a\n\
        prompt for an agent. Ctrl+C always stops it."
)]
pub struct Cli {
    /// Run as if started in <path> (any directory inside a git worktree).
    #[arg(short = 'C', value_name = "path", global = true)]
    pub directory: Option<PathBuf>,

    /// Print the version and exit.
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    pub version: Option<bool>,

    /// Port to listen on (default: 4966, or the next free one; 0 = random).
    #[arg(short = 'p', long, value_name = "port", global = true)]
    pub port: Option<u16>,

    /// Address to bind; use 0.0.0.0 to expose on the network.
    #[arg(short = 'H', long, value_name = "host", default_value = DEFAULT_HOST, global = true)]
    pub host: String,

    /// Trusted public HTTP(S) origin behind a reverse proxy (without a path).
    #[arg(long, value_name = "origin", value_parser = parse_origin, global = true)]
    pub allowed_origin: Option<AllowedOrigin>,

    /// Do not open a browser.
    #[arg(long, global = true)]
    pub no_open: bool,

    /// Keep the server running after all browser tabs close.
    #[arg(long, global = true)]
    pub keep_alive: bool,

    /// Do not watch for changes.
    #[arg(long, global = true)]
    pub no_watch: bool,

    /// Mark matching files viewed for this session (repeatable).
    #[arg(long, value_name = "glob", action = ArgAction::Append, global = true)]
    pub auto_viewed: Vec<String>,

    /// Context lines around changes for this session (default: config, 5).
    #[arg(short = 'U', long, value_name = "n", value_parser = parse_context, global = true)]
    pub context: Option<ContextLines>,

    /// Directory holding the built web client (default: found beside the executable).
    #[arg(long, value_name = "dir", env = "DIFFLE_CLIENT_DIR", global = true)]
    pub client_dir: Option<PathBuf>,

    /// Accepted for scripts written for the Node server; this server starts no language
    /// servers yet.
    #[arg(long, global = true)]
    pub no_lsp: bool,

    /// git-diff style revisions: <rev> | <a>..<b> | <a>...<b> | <a> <b>
    #[arg(value_name = "revs")]
    pub revs: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Shorthands name what to compare, so they are hidden; `config` is the one real action.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Same as `diffle HEAD..worktree`.
    #[command(hide = true)]
    Working,
    /// Same as `diffle <rev>^!`.
    #[command(hide = true)]
    Show {
        /// The commit to review.
        #[arg(default_value = "HEAD")]
        rev: String,
    },
    /// Show or edit the user config (same settings as the UI dialog).
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ConfigAction {
    /// Print the config file path and contents.
    Show,
    /// Add patterns for files that start viewed.
    AddAutoViewed {
        /// Patterns for files that start viewed and collapsed, e.g. "*.lock".
        #[arg(required = true, value_name = "glob")]
        globs: Vec<String>,
    },
    /// Remove patterns for files that start viewed.
    RemoveAutoViewed {
        #[arg(required = true, value_name = "glob")]
        globs: Vec<String>,
    },
    /// Set context lines around changes.
    SetContext {
        #[arg(value_parser = parse_context)]
        n: ContextLines,
    },
    /// Follow moved refs: on, auto (not branches), off.
    SetFollowRefs {
        #[arg(value_parser = parse_follow_refs)]
        mode: FollowRefs,
    },
}

fn parse_origin(text: &str) -> Result<AllowedOrigin, String> {
    AllowedOrigin::parse(text).map_err(|error| error.to_string())
}

fn parse_context(text: &str) -> Result<ContextLines, String> {
    let value: f64 = text
        .parse()
        .map_err(|_| format!("expected a whole number, got {text:?}"))?;
    ContextLines::new(value).map_err(|error| error.to_string())
}

fn parse_follow_refs(text: &str) -> Result<FollowRefs, String> {
    FollowRefs::parse(text).ok_or_else(|| format!("expected on, auto or off, got {text:?}"))
}
