//! `UserConfig` as `config.json` stores it and `/api/config` serves it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::context_lines::{ContextLines, ContextLinesError};
use super::follow_refs::FollowRefs;
use super::language_id::LanguageId;
use super::user_config::{ConfigUpdate, UserConfig, trimmed_globs};

/// A config read from JSON, with what the reader had to drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedConfig {
    pub config: UserConfig,
    /// `lspCommands` keys naming no language diffle knows. A typo would otherwise sit in the
    /// file looking effective.
    pub unknown_languages: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConfigWire {
    auto_viewed: Vec<String>,
    context_lines: u32,
    follow_refs: &'static str,
    lsp_commands: BTreeMap<&'static str, String>,
}

impl UserConfigWire {
    /// The stored config as written to disk.
    #[must_use]
    pub fn stored(config: &UserConfig) -> Self {
        Self::effective(config, &[], config.context_lines)
    }

    /// The stored config plus what this run adds: `--auto-viewed` globs and the live context.
    #[must_use]
    pub fn effective(
        config: &UserConfig,
        extra_auto_viewed: &[String],
        context: ContextLines,
    ) -> Self {
        let mut auto_viewed = config.auto_viewed.clone();
        for glob in extra_auto_viewed {
            if !auto_viewed.contains(glob) {
                auto_viewed.push(glob.clone());
            }
        }
        Self {
            auto_viewed,
            context_lines: context.get(),
            follow_refs: config.follow_refs.as_str(),
            lsp_commands: config
                .lsp_commands
                .iter()
                .map(|(language, command)| (language.as_str(), command.clone()))
                .collect(),
        }
    }
}

/// Reads whatever the file holds, field by field: a field that is missing or of the wrong
/// type takes its default, so a hand-edited file never stops the server.
#[must_use]
pub fn parse_stored(value: &Value) -> ParsedConfig {
    let field = |name: &str| value.get(name);
    let auto_viewed = field("autoViewed")
        .and_then(Value::as_array)
        .map(|globs| trimmed_globs(globs.iter().filter_map(Value::as_str)))
        .unwrap_or_default();
    let context_lines = field("contextLines")
        .and_then(Value::as_f64)
        .map_or_else(ContextLines::default, ContextLines::normalized);
    let follow_refs = field("followRefs")
        .and_then(Value::as_str)
        .and_then(FollowRefs::parse)
        .unwrap_or_default();
    let (lsp_commands, unknown_languages) = lsp_commands(field("lspCommands"));
    ParsedConfig {
        config: UserConfig {
            auto_viewed,
            context_lines,
            follow_refs,
            lsp_commands,
        },
        unknown_languages,
    }
}

fn lsp_commands(value: Option<&Value>) -> (BTreeMap<LanguageId, String>, Vec<String>) {
    let mut known = BTreeMap::new();
    let mut unknown = Vec::new();
    let entries = value.and_then(Value::as_object).into_iter().flatten();
    for (name, command) in entries {
        let Some(command) = command.as_str() else {
            continue;
        };
        match LanguageId::parse(name) {
            Some(language) => {
                known.insert(language, command.trim().to_owned());
            }
            None => unknown.push(name.clone()),
        }
    }
    (known, unknown)
}

/// The body of `PUT /api/config`. Fields it does not name are ignored, as is anything else in it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateWire {
    auto_viewed: Option<Vec<String>>,
    context_lines: Option<f64>,
    follow_refs: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigUpdateError {
    #[error(transparent)]
    ContextLines(#[from] ContextLinesError),
    #[error("followRefs must be \"on\", \"auto\" or \"off\"")]
    FollowRefs,
}

impl TryFrom<ConfigUpdateWire> for ConfigUpdate {
    type Error = ConfigUpdateError;

    fn try_from(wire: ConfigUpdateWire) -> Result<Self, Self::Error> {
        Ok(Self {
            auto_viewed: wire.auto_viewed,
            context_lines: wire.context_lines.map(ContextLines::new).transpose()?,
            follow_refs: wire
                .follow_refs
                .map(|text| FollowRefs::parse(&text).ok_or(ConfigUpdateError::FollowRefs))
                .transpose()?,
        })
    }
}

#[cfg(test)]
#[path = "user_config_wire_tests.rs"]
mod tests;
