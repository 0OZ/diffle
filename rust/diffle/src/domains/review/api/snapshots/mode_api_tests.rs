use serde_json::{Value, json};

use super::*;

fn parsed(body: Value) -> Result<ModeRequest, ApiError> {
    serde_json::from_value::<ModeRequestWire>(body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .try_into()
}

#[test]
fn every_mode_the_client_can_ask_for_reads() {
    assert_eq!(
        parsed(json!({"kind": "working"})).unwrap(),
        ModeRequest::Enter(EntryRequest::Working)
    );
    let spec = Revspec::parse(&["main".to_owned()]).unwrap();
    assert_eq!(
        parsed(json!({"kind": "revspec", "args": ["main"]})).unwrap(),
        ModeRequest::Enter(EntryRequest::Revspec(spec))
    );
}

#[test]
fn a_focus_names_the_range_the_worktree_or_a_commit() {
    let focus = |commit: Value| parsed(json!({"kind": "focus", "commit": commit})).unwrap();
    assert_eq!(focus(Value::Null), ModeRequest::Focus(FocusTarget::Range));
    assert_eq!(
        focus(json!("worktree")),
        ModeRequest::Focus(FocusTarget::Worktree)
    );
    assert_eq!(
        focus(json!("abc123")),
        ModeRequest::Focus(FocusTarget::Commit("abc123".to_owned()))
    );
}

#[test]
fn bad_revisions_are_the_callers_and_missing_features_are_said_so() {
    let refused = parsed(json!({"kind": "revspec", "args": []})).unwrap_err();
    assert_eq!(refused.status(), axum::http::StatusCode::BAD_REQUEST);
    for kind in ["pr", "interdiff", "range-diff", "pair"] {
        let missing = parsed(json!({"kind": kind, "args": [], "from": 1, "to": 2, "commit": null}))
            .unwrap_err();
        assert_eq!(
            missing.status(),
            axum::http::StatusCode::NOT_IMPLEMENTED,
            "{kind}"
        );
    }
    assert!(parsed(json!({"kind": "teleport"})).is_err());
}
