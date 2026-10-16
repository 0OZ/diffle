use super::*;
use serde_json::json;

fn line_anchor() -> Value {
    json!({"path": "a.py", "side": "new", "startLine": 2, "endLine": 2, "quoted": "b"})
}

fn viewed() -> Value {
    json!([{"path": "a.py", "blob": "sha", "viewed": true}])
}

#[test]
fn unknown_shapes_read_as_an_empty_store() {
    for text in [
        "{}",
        "null",
        "[]",
        r#"{"version":4,"sets":{}}"#,
        r#"{"version":3}"#,
        r#"{"version":2,"sets":[]}"#,
    ] {
        assert_eq!(
            StoredFile::parse(text).unwrap(),
            StoredFile::empty(),
            "{text}"
        );
    }
    assert!(StoredFile::parse("{not json").is_err());
    assert!(StoredFile::empty().is_current());
}

#[test]
fn a_current_file_upgrades_to_itself() {
    let text =
        json!({"version": 3, "sets": {"working": {"threads": [], "viewed": []}}}).to_string();
    let file = StoredFile::parse(&text).unwrap();
    assert_eq!(file.version(), 3);
    assert_eq!(file.clone().upgrade().unwrap(), file);
}

#[test]
fn v1_comments_become_single_message_unresolved_threads() {
    let comment = json!({
        "id": "c1", "anchor": {"kind": "line", "path": "a.py", "side": "new", "startLine": 2, "endLine": 2, "quoted": "b"},
        "body": "old note", "createdAt": 5, "updatedAt": 6, "stale": true, "staleFromLine": 9
    });
    let text = json!({"version": 1, "sets": {"working": {"comments": [comment], "viewed": viewed()}, "empty": {}}});
    let file = StoredFile::parse(&text.to_string()).unwrap();
    assert_eq!(file.version(), 1);
    let upgraded = file.upgrade().unwrap();
    assert!(upgraded.is_current());
    let set = upgraded.set("working").unwrap();
    let thread = &set.threads[0];
    assert_eq!(
        (
            thread.id.as_str(),
            thread.resolved,
            thread.stale,
            thread.stale_from_line
        ),
        ("c1", false, true, Some(9))
    );
    assert_eq!(
        thread.anchor,
        AnchorWire::Line(serde_json::from_value(line_anchor()).unwrap())
    );
    let message = &thread.messages[0];
    assert_eq!(
        (
            message.body.as_str(),
            message.created_at,
            message.updated_at
        ),
        ("old note", 5, 6)
    );
    assert_eq!(serde_json::to_value(&set.viewed).unwrap(), viewed());
    assert_eq!(upgraded.set("empty").unwrap(), SetStore::default());
}

#[test]
fn v2_threads_get_their_line_anchors_stamped() {
    let thread = json!({
        "id": "t1", "anchor": line_anchor(),
        "messages": [{"id": "m1", "body": "note", "createdAt": 5, "updatedAt": 6}],
        "resolved": true, "resolvedAt": 7, "stale": false
    });
    let text = json!({"version": 2, "sets": {"working": {"threads": [thread], "viewed": viewed()}, "pr:abc": {"threads": []}}});
    let upgraded = StoredFile::parse(&text.to_string())
        .unwrap()
        .upgrade()
        .unwrap();
    let rendered: Value = serde_json::from_str(&upgraded.render().unwrap()).unwrap();
    let mut anchor = line_anchor();
    anchor["kind"] = json!("line");
    assert_eq!(rendered["version"], 3);
    assert_eq!(rendered["sets"]["working"]["threads"][0]["anchor"], anchor);
    assert_eq!(rendered["sets"]["working"]["threads"][0]["resolvedAt"], 7);
    assert_eq!(rendered["sets"]["pr:abc"]["viewed"], json!([]));
}

#[test]
fn replacing_one_set_leaves_every_other_untouched() {
    let text =
        json!({"version": 3, "sets": {"other": {"threads": [], "viewed": [], "note": "kept"}}});
    let file = StoredFile::parse(&text.to_string()).unwrap();
    let set = SetStore {
        threads: Vec::new(),
        viewed: serde_json::from_value(viewed()).unwrap(),
    };
    let rendered: Value =
        serde_json::from_str(&file.with_set("working", &set).unwrap().render().unwrap()).unwrap();
    assert_eq!(rendered["sets"]["other"]["note"], "kept");
    assert_eq!(rendered["sets"]["working"]["viewed"], viewed());
    assert!(
        rendered["sets"]["working"]["threads"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn rendering_is_indented_by_two_spaces() {
    let text = StoredFile::empty().render().unwrap();
    assert_eq!(text, "{\n  \"version\": 3,\n  \"sets\": {}\n}");
}

#[test]
fn a_stored_set_that_breaks_a_domain_rule_is_refused() {
    let set: SetStore = serde_json::from_value(json!({
        "threads": [], "viewed": [{"path": "", "blob": "s", "viewed": true}]
    }))
    .unwrap();
    assert_eq!(
        CommentSet::try_from(set),
        Err(StoredDataError::Viewed(EmptyPath))
    );
    let empty = CommentSet::try_from(SetStore::default()).unwrap();
    assert_eq!(SetStore::from(&empty), SetStore::default());
}
