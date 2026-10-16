//! The composition root: a run, step by step. It names each part and holds no logic of its
//! own — read it top to bottom and you have read what `diffle` does.
//!
//! 1. [`cli`] reads the command line; [`config`] turns it into domain types.
//! 2. The stored preferences are opened.
//! 3. Either [`settings_command`] edits them, or [`serving`] runs one review: resolve it,
//!    listen, open the browser, wait for its end, hand the open comments over on stdout.
//! 4. [`app_error`] says how a failure is spelled and which exit code it earns.

mod app_error;
mod cli;
mod config;
mod serving;
mod settings_command;

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use crate::domains::settings::config_store::{ConfigStore, default_config_path};
use crate::kernel::logging;
use app_error::AppError;
use cli::Cli;
use config::Invocation;

pub async fn run() -> ExitCode {
    logging::init();
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("✖ {error}");
            error.exit_code()
        }
    }
}

async fn execute(cli: Cli) -> Result<(), AppError> {
    let invocation = Invocation::from_cli(cli)?;
    let file = default_config_path(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )?;
    let settings = Arc::new(ConfigStore::open(file).await?);
    match invocation {
        Invocation::Review(config) => serving::serve_review::serve_review(*config, settings).await,
        Invocation::Settings(action) => settings_command::run(&settings, action).await,
    }
}
