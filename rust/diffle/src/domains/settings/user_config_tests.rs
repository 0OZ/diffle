use super::*;

#[test]
fn an_update_changes_only_the_fields_it_names() {
    let before = UserConfig {
        auto_viewed: vec!["a".to_owned()],
        follow_refs: FollowRefs::On,
        lsp_commands: BTreeMap::from([(LanguageId::Go, "gopls".to_owned())]),
        ..UserConfig::default()
    };
    let after = before.updated(&ConfigUpdate {
        auto_viewed: Some(vec![" b ".to_owned(), String::new()]),
        context_lines: Some(ContextLines::new(9.0).unwrap()),
        follow_refs: None,
    });
    assert_eq!(after.auto_viewed, ["b"]);
    assert_eq!(after.context_lines.get(), 9);
    assert_eq!(after.follow_refs, FollowRefs::On);
    assert_eq!(after.lsp_commands, before.lsp_commands);
    assert_eq!(before.updated(&ConfigUpdate::default()), before);
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let config = UserConfig::default();
    assert!(config.auto_viewed.is_empty() && config.lsp_commands.is_empty());
    assert_eq!(config.context_lines.get(), 5);
    assert_eq!(config.follow_refs, FollowRefs::Auto);
}
