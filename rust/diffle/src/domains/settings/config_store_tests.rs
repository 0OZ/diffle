use super::*;
use crate::domains::settings::context_lines::ContextLines;
use crate::domains::settings::follow_refs::FollowRefs;

fn context(lines: f64) -> ConfigUpdate {
    ConfigUpdate {
        context_lines: Some(ContextLines::new(lines).unwrap()),
        ..ConfigUpdate::default()
    }
}

#[test]
fn the_path_prefers_xdg_then_the_home_config_directory() {
    let os = |text: &str| Some(OsString::from(text));
    assert_eq!(
        default_config_path(os("/x"), os("/home/me")).unwrap(),
        Path::new("/x/diffle/config.json")
    );
    assert_eq!(
        default_config_path(os(""), os("/home/me")).unwrap(),
        Path::new("/home/me/.config/diffle/config.json")
    );
    assert_eq!(
        default_config_path(None, os("/home/me")).unwrap(),
        Path::new("/home/me/.config/diffle/config.json")
    );
    assert!(matches!(
        default_config_path(None, os("")),
        Err(ConfigError::NoConfigDirectory)
    ));
}

#[tokio::test]
async fn a_missing_file_is_the_default_and_a_set_persists_across_opens() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("diffle/config.json");
    let store = ConfigStore::open(file.clone()).await.unwrap();
    assert_eq!(store.get(), UserConfig::default());

    let saved = store.set(&context(3.0)).await.unwrap();
    assert_eq!(saved.context_lines.get(), 3);
    assert_eq!(store.get(), saved);

    let reopened = ConfigStore::open(file.clone()).await.unwrap();
    assert_eq!(reopened.get(), saved);
    assert!(std::fs::read_to_string(&file).unwrap().ends_with("}\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn the_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    let store = ConfigStore::open(file.clone()).await.unwrap();
    store.set(&ConfigUpdate::default()).await.unwrap();
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[tokio::test]
async fn a_malformed_file_falls_back_to_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    std::fs::write(&file, "{ not json").unwrap();
    let store = ConfigStore::open(file).await.unwrap();
    assert_eq!(store.get(), UserConfig::default());
}

#[tokio::test]
async fn overlapping_sets_land_in_call_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::open(dir.path().join("config.json"))
        .await
        .unwrap();
    let follow = ConfigUpdate {
        follow_refs: Some(FollowRefs::On),
        ..ConfigUpdate::default()
    };
    let wider = context(8.0);
    let (first, second) = tokio::join!(store.set(&follow), store.set(&wider));
    first.unwrap();
    let last = second.unwrap();
    assert_eq!(
        (last.follow_refs, last.context_lines.get()),
        (FollowRefs::On, 8)
    );
}

#[tokio::test]
async fn a_failed_save_leaves_the_stored_config_alone() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    let store = ConfigStore::open(blocker.join("config.json"))
        .await
        .unwrap();
    // A file where the config directory should be makes the save fail.
    std::fs::write(&blocker, "x").unwrap();
    let error = store.set(&context(1.0)).await.unwrap_err();
    assert!(matches!(error, ConfigError::Save { .. }));
    assert_eq!(store.get(), UserConfig::default());
}

#[tokio::test]
async fn an_unreadable_path_is_an_error_not_a_default() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let error = ConfigStore::open(blocker.join("config.json"))
        .await
        .unwrap_err();
    assert!(matches!(error, ConfigError::Read { .. }));
}

#[tokio::test]
async fn render_is_the_text_the_file_holds() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.json");
    let store = ConfigStore::open(file.clone()).await.unwrap();
    store.set(&context(8.0)).await.unwrap();
    assert_eq!(
        store.render().unwrap(),
        std::fs::read_to_string(&file).unwrap()
    );
    assert!(store.render().unwrap().contains("\"contextLines\": 8"));
}
