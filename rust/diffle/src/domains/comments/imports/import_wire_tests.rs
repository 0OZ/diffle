use super::*;
use crate::domains::comments::sample_threads::{line, path, range};
use serde_json::json;

fn lines(side: Side, start: u64, end: u64, quoted: Option<&str>) -> ThreadTarget {
    ThreadTarget::Lines {
        side,
        range: range(start, end),
        quoted: quoted.map(Quote::new),
    }
}

fn new_thread(file: &str, target: ThreadTarget, body: &str) -> NewThread {
    NewThread::new(path(file), target, Body::parse(body).unwrap())
}

fn error_of(payload: Value) -> String {
    parse_imports_value(payload).unwrap_err().to_string()
}

#[test]
fn accepts_one_object_or_an_array_as_json_text_or_a_value_and_keeps_only_known_fields() {
    let one = json!({"path": "a.py", "startLine": 3, "body": "x", "extra": 1});
    let expected_one = new_thread("a.py", lines(Side::New, 3, 3, None), "x");
    assert_eq!(
        parse_imports(&one.to_string()).unwrap(),
        std::slice::from_ref(&expected_one)
    );
    let both = json!([
        one,
        {"path": "b.py", "side": "old", "startLine": 1, "endLine": 2, "body": "y", "author": "agent", "quoted": "q"}
    ]);
    assert_eq!(
        parse_imports_value(both).unwrap(),
        [
            expected_one,
            new_thread("b.py", lines(Side::Old, 1, 2, Some("q")), "y")
        ]
    );
    assert_eq!(parse_imports_value(json!([])).unwrap(), []);
    assert_eq!(line(3).get(), 3);
}

#[test]
fn a_payload_without_a_line_is_a_comment_on_the_whole_file_and_line_fields_need_a_line() {
    assert_eq!(
        parse_imports_value(json!({"path": "a.py", "body": "Split this."})).unwrap(),
        [new_thread("a.py", ThreadTarget::File, "Split this.")]
    );
    for extra in [
        json!({"side": "new"}),
        json!({"endLine": 2}),
        json!({"quoted": "q"}),
    ] {
        let mut payload = json!({"path": "a.py", "body": "x"});
        payload
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(
            error_of(payload),
            format!("invalid comment: startLine: {NEEDS_START_LINE}")
        );
    }
}

#[test]
fn rejects_malformed_json_text_and_oversized_payloads() {
    assert!(matches!(
        parse_imports("{not json"),
        Err(ImportError::Json(_))
    ));
    assert!(
        parse_imports("{not json")
            .unwrap_err()
            .to_string()
            .starts_with("invalid comment JSON: ")
    );
    let big = format!("\"{}\"", "x".repeat(MAX_PAYLOAD_BYTES));
    assert_eq!(
        parse_imports(&big).unwrap_err().to_string(),
        "comment payload over 1 MiB"
    );
}

#[test]
fn names_the_offending_entry_only_when_there_are_several() {
    let bad = json!({"path": "a.py", "startLine": 0, "body": "x"});
    let ok = json!({"path": "a.py", "startLine": 1, "body": "ok"});
    assert_eq!(
        error_of(json!([ok, bad.clone()])),
        "invalid comment[1]: startLine: Too small: expected number to be >0"
    );
    assert_eq!(
        error_of(json!([bad])),
        "invalid comment: startLine: Too small: expected number to be >0"
    );
}

#[test]
fn reports_each_rule_with_the_validators_words() {
    let with = |extra: Value| {
        let mut payload = json!({"path": "a.py", "body": "x"});
        payload
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        error_of(payload)
    };
    assert_eq!(
        with(json!({"startLine": 2, "endLine": 1})),
        "invalid comment: endLine: endLine must be ≥ startLine"
    );
    assert_eq!(
        with(json!({"body": "  "})),
        "invalid comment: body: body required"
    );
    assert_eq!(
        with(json!({"side": "left"})),
        "invalid comment: side: Invalid option: expected one of \"old\"|\"new\""
    );
    assert_eq!(
        with(json!({"startLine": 1.5})),
        "invalid comment: startLine: Invalid input: expected int, received number"
    );
    assert_eq!(
        with(json!({"startLine": "1"})),
        "invalid comment: startLine: Invalid input: expected number, received string"
    );
    assert_eq!(
        with(json!({"startLine": null})),
        "invalid comment: startLine: Invalid input: expected number, received null"
    );
    assert_eq!(
        with(json!({"quoted": 5})),
        "invalid comment: quoted: Invalid input: expected string, received number"
    );
    assert_eq!(
        with(json!({"startLine": 9_007_199_254_740_993_u64})),
        "invalid comment: startLine: Too big: expected int to be <=9007199254740991"
    );
    assert_eq!(
        parse_imports_value(json!({"path": "a", "startLine": 2.0, "body": "x"}))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn reports_wrong_shapes_and_every_field_in_schema_order() {
    assert_eq!(
        error_of(json!("a string")),
        "invalid comment: : Invalid input: expected object, received string"
    );
    assert_eq!(
        error_of(json!(null)),
        "invalid comment: : Invalid input: expected object, received null"
    );
    assert_eq!(
        error_of(json!([[]])),
        "invalid comment: : Invalid input: expected object, received array"
    );
    assert_eq!(
        error_of(json!({})),
        "invalid comment: path: Invalid input: expected string, received undefined; \
         body: Invalid input: expected string, received undefined"
    );
    assert_eq!(
        error_of(json!({"path": "", "body": " ", "startLine": 2.5})),
        "invalid comment: path: Too small: expected string to have >=1 characters; \
         startLine: Invalid input: expected int, received number; body: body required"
    );
}

#[test]
fn a_too_small_value_still_meets_the_cross_field_checks_but_a_wrong_type_stops_them() {
    assert_eq!(
        error_of(json!({"path": "a", "body": "x", "startLine": 5, "endLine": 0})),
        "invalid comment: endLine: Too small: expected number to be >0; endLine: endLine must be ≥ startLine"
    );
    assert_eq!(
        error_of(json!({"path": "a", "body": "x", "side": "new", "endLine": 0})),
        format!("invalid comment: endLine: {NOT_POSITIVE}; startLine: {NEEDS_START_LINE}")
    );
    assert_eq!(
        error_of(json!({"path": "a", "body": "x", "startLine": 2, "endLine": 1, "side": "zz"})),
        format!("invalid comment: side: {BAD_SIDE}")
    );
}
