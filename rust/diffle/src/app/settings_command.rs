//! `diffle config`: print or edit the stored preferences. Every action answers with the text
//! it prints, so the output is what a test judges.

use super::app_error::AppError;
use super::cli::ConfigAction;
use crate::domains::settings::config_store::ConfigStore;
use crate::domains::settings::user_config::ConfigUpdate;

pub async fn run(store: &ConfigStore, action: ConfigAction) -> Result<(), AppError> {
    println!("{}", apply(store, action).await?);
    Ok(())
}

/// What the action prints, after storing what it changes.
pub async fn apply(store: &ConfigStore, action: ConfigAction) -> Result<String, AppError> {
    match action {
        ConfigAction::Show => Ok(format!(
            "# {}\n{}",
            store.file().display(),
            store.render()?.trim_end()
        )),
        ConfigAction::AddAutoViewed { globs } => {
            let mut patterns = store.get().auto_viewed;
            for glob in globs {
                if !patterns.contains(&glob) {
                    patterns.push(glob);
                }
            }
            auto_viewed(store, patterns).await
        }
        ConfigAction::RemoveAutoViewed { globs } => {
            let mut patterns = store.get().auto_viewed;
            patterns.retain(|pattern| !globs.contains(pattern));
            auto_viewed(store, patterns).await
        }
        ConfigAction::SetContext { n } => {
            let stored = store
                .set(&ConfigUpdate {
                    context_lines: Some(n),
                    ..ConfigUpdate::default()
                })
                .await?;
            Ok(stored.context_lines.get().to_string())
        }
        ConfigAction::SetFollowRefs { mode } => {
            let stored = store
                .set(&ConfigUpdate {
                    follow_refs: Some(mode),
                    ..ConfigUpdate::default()
                })
                .await?;
            Ok(stored.follow_refs.to_string())
        }
    }
}

async fn auto_viewed(store: &ConfigStore, patterns: Vec<String>) -> Result<String, AppError> {
    let stored = store
        .set(&ConfigUpdate {
            auto_viewed: Some(patterns),
            ..ConfigUpdate::default()
        })
        .await?;
    Ok(stored.auto_viewed.join("\n"))
}

#[cfg(test)]
#[path = "settings_command_tests.rs"]
mod tests;
