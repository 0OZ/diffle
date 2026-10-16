//! What a run was asked to do, validated: the command line parsed into domain types.

use std::path::{Path, PathBuf};

use super::cli::{Cli, Command, ConfigAction};
use crate::domains::revisions::entry_request::EntryRequest;
use crate::domains::revisions::revspec::Revspec;
use crate::domains::revisions::revspec_error::RevspecError;
use crate::domains::settings::context_lines::ContextLines;
use crate::kernel::allowed_origin::AllowedOrigin;
use crate::workers::http_server::listening::bind::PortChoice;

const CURRENT_DIRECTORY: &str = ".";

/// What `diffle` does with this command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// Serve a review.
    Review(Box<Config>),
    /// Print or edit the stored preferences, then exit.
    Settings(ConfigAction),
}

/// Whether a browser is opened for the review, and what its closing means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    /// `--no-open`: the review ends only on a signal.
    NotOpened,
    /// `--keep-alive`: opened, but closing the tab does not end the review.
    Kept,
    /// The default: the review ends when the last tab it opened closes.
    EndsWithTab,
}

impl Browser {
    #[must_use]
    pub const fn opens(self) -> bool {
        match self {
            Self::NotOpened => false,
            Self::Kept | Self::EndsWithTab => true,
        }
    }

    #[must_use]
    pub const fn ends_with_tab(self) -> bool {
        match self {
            Self::EndsWithTab => true,
            Self::NotOpened | Self::Kept => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    directory: PathBuf,
    request: EntryRequest,
    host: String,
    port: PortChoice,
    allowed_origin: Option<AllowedOrigin>,
    browser: Browser,
    watch: bool,
    auto_viewed: Vec<String>,
    context: Option<ContextLines>,
    client_dir: Option<PathBuf>,
}

impl Invocation {
    pub fn from_cli(cli: Cli) -> Result<Self, RevspecError> {
        let Cli {
            directory,
            port,
            host,
            allowed_origin,
            no_open,
            keep_alive,
            no_watch,
            auto_viewed,
            context,
            client_dir,
            revs,
            command,
            version: _,
            no_lsp: _,
        } = cli;
        let request = match command {
            Some(Command::Config { action }) => {
                return Ok(Self::Settings(action.unwrap_or(ConfigAction::Show)));
            }
            Some(Command::Working) => EntryRequest::Working,
            Some(Command::Show { rev }) => {
                EntryRequest::Revspec(Revspec::parse(&[format!("{rev}^!")])?)
            }
            None if revs.is_empty() => EntryRequest::Working,
            None => EntryRequest::Revspec(Revspec::parse(&revs)?),
        };
        let browser = match (no_open, keep_alive) {
            (true, _) => Browser::NotOpened,
            (false, true) => Browser::Kept,
            (false, false) => Browser::EndsWithTab,
        };
        Ok(Self::Review(Box::new(Config {
            directory: directory.unwrap_or_else(|| PathBuf::from(CURRENT_DIRECTORY)),
            request,
            host,
            port: port.map_or(PortChoice::Probe, PortChoice::Exact),
            allowed_origin,
            browser,
            watch: !no_watch,
            auto_viewed,
            context,
            client_dir,
        })))
    }
}

impl Config {
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn request(&self) -> &EntryRequest {
        &self.request
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> PortChoice {
        self.port
    }

    pub fn allowed_origin(&self) -> Option<&AllowedOrigin> {
        self.allowed_origin.as_ref()
    }

    pub fn browser(&self) -> Browser {
        self.browser
    }

    pub fn watch(&self) -> bool {
        self.watch
    }

    pub fn auto_viewed(&self) -> &[String] {
        &self.auto_viewed
    }

    /// `-U`; the stored preference decides when absent.
    pub fn context(&self) -> Option<ContextLines> {
        self.context
    }

    pub fn client_dir(&self) -> Option<&Path> {
        self.client_dir.as_deref()
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
