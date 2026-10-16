use serde_json::json;

use super::*;

#[test]
fn an_empty_object_is_the_default() {
    let parsed = parse_stored(&json!({}));
    assert_eq!(parsed.config, UserConfig::default());
    assert!(parsed.unknown_languages.is_empty());
}

#[test]
fn reads_each_field_and_defaults_the_unusable_ones() {
    let parsed = parse_stored(&json!({
        "autoViewed": ["  *.lock ", "", 7, "docs/**"],
        "contextLines": 12.7,
        "followRefs": "off",
    }));
    assert_eq!(parsed.config.auto_viewed, ["*.lock", "docs/**"]);
    assert_eq!(parsed.config.context_lines.get(), 12);
    assert_eq!(parsed.config.follow_refs, FollowRefs::Off);

    let broken = parse_stored(&json!({
        "autoViewed": "*.lock",
        "contextLines": "many",
        "followRefs": "sometimes",
    }));
    assert_eq!(broken.config, UserConfig::default());
    assert_eq!(
        parse_stored(&json!(["not", "an", "object"])).config,
        UserConfig::default()
    );
}

#[test]
fn keeps_commands_for_known_languages_and_reports_the_rest() {
    let parsed = parse_stored(&json!({
        "lspCommands": {"python": " pyrefly lsp ", "rust": "", "klingon": "qapla", "go": 3},
    }));
    let commands = &parsed.config.lsp_commands;
    assert_eq!(
        commands.get(&LanguageId::Python).map(String::as_str),
        Some("pyrefly lsp")
    );
    assert_eq!(
        commands.get(&LanguageId::Rust).map(String::as_str),
        Some("")
    );
    assert_eq!(commands.len(), 2);
    assert_eq!(parsed.unknown_languages, ["klingon"]);
}

#[test]
fn serializes_with_the_wire_names_and_round_trips() {
    let config = parse_stored(&json!({"lspCommands": {"go": "gopls"}, "followRefs": "on"})).config;
    let wire = serde_json::to_value(UserConfigWire::stored(&config)).unwrap();
    assert_eq!(
        wire,
        json!({"autoViewed": [], "contextLines": 5, "followRefs": "on", "lspCommands": {"go": "gopls"}})
    );
    assert_eq!(parse_stored(&wire).config, config);
}

#[test]
fn the_effective_config_adds_the_runs_globs_once_and_the_live_context() {
    let config = parse_stored(&json!({"autoViewed": ["a", "b"]})).config;
    let extra = ["b".to_owned(), "c".to_owned()];
    let wire = serde_json::to_value(UserConfigWire::effective(
        &config,
        &extra,
        ContextLines::new(20.0).unwrap(),
    ))
    .unwrap();
    assert_eq!(wire["autoViewed"], json!(["a", "b", "c"]));
    assert_eq!(wire["contextLines"], 20);
}

#[test]
fn an_update_body_is_validated() {
    let update = |body: Value| -> Result<ConfigUpdate, ConfigUpdateError> {
        serde_json::from_value::<ConfigUpdateWire>(body)
            .unwrap()
            .try_into()
    };
    let ok = update(json!({"contextLines": 3, "followRefs": "off", "extra": true})).unwrap();
    assert_eq!(ok.context_lines.map(ContextLines::get), Some(3));
    assert_eq!(ok.follow_refs, Some(FollowRefs::Off));
    assert_eq!(update(json!({})).unwrap(), ConfigUpdate::default());
    assert_eq!(
        update(json!({"contextLines": 1.5})).unwrap_err(),
        ContextLinesError.into()
    );
    assert_eq!(
        update(json!({"followRefs": "never"})).unwrap_err(),
        ConfigUpdateError::FollowRefs
    );
    assert!(serde_json::from_value::<ConfigUpdateWire>(json!({"contextLines": "3"})).is_err());
}
