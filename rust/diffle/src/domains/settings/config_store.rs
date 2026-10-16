//! `$XDG_CONFIG_HOME/diffle/config.json`: machine-wide, not per repository.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

use thiserror::Error;
use tokio::sync::Mutex;

use super::user_config::{ConfigUpdate, UserConfig};
use super::user_config_wire::{UserConfigWire, parse_stored};
use crate::kernel::atomic_file::write_file_atomic;

const APP_DIR: &str = "diffle";
const FILE_NAME: &str = "config.json";
const HOME_CONFIG_DIR: &str = ".config";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot save {file}: {source}")]
    Save { file: PathBuf, source: io::Error },
    #[error("cannot encode the config: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("cannot read {file}: {source}")]
    Read { file: PathBuf, source: io::Error },
    #[error("neither XDG_CONFIG_HOME nor HOME is set, so there is nowhere to keep the config")]
    NoConfigDirectory,
}

/// Where the config lives: under `XDG_CONFIG_HOME` when set and non-empty, else `~/.config`.
pub fn default_config_path(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, ConfigError> {
    let base = match (
        xdg_config_home.filter(|dir| !dir.is_empty()),
        home.filter(|dir| !dir.is_empty()),
    ) {
        (Some(xdg), _) => PathBuf::from(xdg),
        (None, Some(home)) => PathBuf::from(home).join(HOME_CONFIG_DIR),
        (None, None) => return Err(ConfigError::NoConfigDirectory),
    };
    Ok(base.join(APP_DIR).join(FILE_NAME))
}

/// The stored config. Reads are immediate; writes queue, so overlapping updates land in call order.
#[derive(Debug)]
pub struct ConfigStore {
    file: PathBuf,
    config: RwLock<UserConfig>,
    writing: Mutex<()>,
}

impl ConfigStore {
    /// Reads the file; a missing one is the default config, an unreadable or malformed one is
    /// reported and replaced by the default rather than stopping the server.
    pub async fn open(file: PathBuf) -> Result<Self, ConfigError> {
        let config = match tokio::fs::read_to_string(&file).await {
            Ok(text) => parse(&file, &text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => UserConfig::default(),
            Err(source) => return Err(ConfigError::Read { file, source }),
        };
        Ok(Self {
            file,
            config: RwLock::new(config),
            writing: Mutex::new(()),
        })
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn get(&self) -> UserConfig {
        self.config
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The stored config as the file spells it.
    pub fn render(&self) -> Result<String, ConfigError> {
        Ok(encode(&self.get())?)
    }

    /// Merges `update` in and persists it. The stored config changes only once the file is
    /// written, so a failed save leaves memory and disk agreeing.
    pub async fn set(&self, update: &ConfigUpdate) -> Result<UserConfig, ConfigError> {
        let _queued = self.writing.lock().await;
        let next = self.get().updated(update);
        let text = encode(&next)?;
        // May hold a shell command, so the file is owner-only.
        write_file_atomic(&self.file, &text)
            .await
            .map_err(|source| ConfigError::Save {
                file: self.file.clone(),
                source,
            })?;
        *self.config.write().unwrap_or_else(PoisonError::into_inner) = next.clone();
        Ok(next)
    }
}

/// The file's text: pretty JSON ending in a newline.
fn encode(config: &UserConfig) -> Result<String, serde_json::Error> {
    let mut text = serde_json::to_string_pretty(&UserConfigWire::stored(config))?;
    text.push('\n');
    Ok(text)
}

fn parse(file: &Path, text: &str) -> UserConfig {
    let value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(file = %file.display(), %error, "ignoring unreadable config");
            return UserConfig::default();
        }
    };
    let parsed = parse_stored(&value);
    for language in &parsed.unknown_languages {
        tracing::warn!(%language, "ignoring config lspCommands entry: not a language diffle knows");
    }
    parsed.config
}

#[cfg(test)]
#[path = "config_store_tests.rs"]
mod tests;
