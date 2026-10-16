//! The preferences a reviewer sets once for the machine.

use std::collections::BTreeMap;

use super::context_lines::ContextLines;
use super::follow_refs::FollowRefs;
use super::language_id::LanguageId;

/// What is stored in `config.json`. Every field has a default, so a file may be partial.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserConfig {
    /// Globs for files that start viewed and collapsed.
    pub auto_viewed: Vec<String>,
    pub context_lines: ContextLines,
    pub follow_refs: FollowRefs,
    /// Language server command per language, replacing the built-in candidate; an empty
    /// command turns the language off.
    pub lsp_commands: BTreeMap<LanguageId, String>,
}

/// A change to the stored config; a field left out stays as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigUpdate {
    pub auto_viewed: Option<Vec<String>>,
    pub context_lines: Option<ContextLines>,
    pub follow_refs: Option<FollowRefs>,
}

impl UserConfig {
    /// This config with `update` merged in.
    #[must_use]
    pub fn updated(&self, update: &ConfigUpdate) -> Self {
        Self {
            auto_viewed: update.auto_viewed.as_deref().map_or_else(
                || self.auto_viewed.clone(),
                |globs| trimmed_globs(globs.iter().map(String::as_str)),
            ),
            context_lines: update.context_lines.unwrap_or(self.context_lines),
            follow_refs: update.follow_refs.unwrap_or(self.follow_refs),
            lsp_commands: self.lsp_commands.clone(),
        }
    }
}

/// Globs as the reviewer typed them, minus the blanks.
pub fn trimmed_globs<'a>(globs: impl Iterator<Item = &'a str>) -> Vec<String> {
    globs
        .map(str::trim)
        .filter(|glob| !glob.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "user_config_tests.rs"]
mod tests;
