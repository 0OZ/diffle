use super::*;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

async fn store() -> (ConfigStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::open(dir.path().join("config.json"))
        .await
        .unwrap();
    (store, dir)
}

fn globs(patterns: &[&str]) -> Vec<String> {
    patterns
        .iter()
        .map(|pattern| (*pattern).to_owned())
        .collect()
}

#[tokio::test]
async fn show_names_the_file_and_prints_the_stored_config() {
    let (store, dir) = store().await;
    let shown = apply(&store, ConfigAction::Show).await.unwrap();
    let (header, body) = shown.split_once('\n').unwrap();
    assert_eq!(
        header,
        format!("# {}", dir.path().join("config.json").display())
    );
    assert!(body.starts_with('{') && body.ends_with('}'), "{body}");
    assert!(body.contains("\"contextLines\": 5"), "{body}");
}

#[tokio::test]
async fn added_patterns_keep_their_order_and_never_repeat() {
    let (store, _dir) = store().await;
    let add = |patterns: &[&str]| ConfigAction::AddAutoViewed {
        globs: globs(patterns),
    };
    assert_eq!(
        apply(&store, add(&["*.lock", "dist/**"])).await.unwrap(),
        "*.lock\ndist/**"
    );
    assert_eq!(
        apply(&store, add(&["dist/**", "*.snap"])).await.unwrap(),
        "*.lock\ndist/**\n*.snap"
    );
}

#[tokio::test]
async fn removed_patterns_leave_the_rest() {
    let (store, _dir) = store().await;
    apply(
        &store,
        ConfigAction::AddAutoViewed {
            globs: globs(&["a", "b", "c"]),
        },
    )
    .await
    .unwrap();
    let left = apply(
        &store,
        ConfigAction::RemoveAutoViewed {
            globs: globs(&["b", "nope"]),
        },
    )
    .await
    .unwrap();
    assert_eq!(left, "a\nc");
}

#[tokio::test]
async fn context_and_follow_refs_are_stored_and_echoed() {
    let (store, _dir) = store().await;
    let context = ContextLines::new(12.0).unwrap();
    assert_eq!(
        apply(&store, ConfigAction::SetContext { n: context })
            .await
            .unwrap(),
        "12"
    );
    assert_eq!(
        apply(
            &store,
            ConfigAction::SetFollowRefs {
                mode: FollowRefs::Off
            }
        )
        .await
        .unwrap(),
        "off"
    );
    let stored = store.get();
    assert_eq!(
        (stored.context_lines.get(), stored.follow_refs),
        (12, FollowRefs::Off)
    );
}

#[tokio::test]
async fn a_failed_save_is_reported_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    let store = ConfigStore::open(blocker.join("config.json"))
        .await
        .unwrap();
    std::fs::write(&blocker, "x").unwrap();
    let error = apply(
        &store,
        ConfigAction::AddAutoViewed {
            globs: globs(&["a"]),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, AppError::Settings(_)), "{error}");
    assert!(store.get().auto_viewed.is_empty());
}
