//! One run of the program: read the command line, open the preferences, do what was asked.

use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use super::app_error::AppError;
use super::cli::Cli;
use super::config::Invocation;
use super::{serving, settings_command};
use crate::domains::settings::config_store::{ConfigStore, default_config_path};
use crate::kernel::logging;

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
