use super::*;
use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::comment_error::Unquotable;
use crate::domains::comments::imports::anchor_answers::{AnchorQuestions, QuoteQuestion};
use crate::domains::comments::imports::new_thread::ThreadTarget;
use crate::domains::comments::sample_threads::{path, range};
use crate::domains::comments::storage::scratch_store::{
    body, file_anchor, first_bodies_of, open, request, snapshot_answers,
};
use crate::kernel::side::Side;

#[tokio::test]
async fn imports_payloads_quoting_from_the_snapshot_and_skipping_open_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let first_batch = vec![request(1, 1, "one", None)];
    let first = store
        .import_threads(first_batch.clone(), &snapshot_answers(&first_batch))
        .await
        .unwrap();
    assert_eq!(first.skipped, 0);
    let Anchor::Line(placed) = first.added[0].anchor() else {
        panic!("a line thread")
    };
    assert_eq!(placed.quoted().as_str(), "L1-1");
    assert_eq!(first.added[0].messages().first().text().as_str(), "one");

    let batch = vec![
        request(1, 1, "one", None),
        request(2, 3, "two", Some("given")),
        request(2, 3, "two", None),
    ];
    let second = store
        .import_threads(batch.clone(), &snapshot_answers(&batch))
        .await
        .unwrap();
    assert_eq!(second.skipped, 2);
    let quoted: Vec<String> = second
        .added
        .iter()
        .filter_map(|t| match t.anchor() {
            Anchor::Line(line) => Some(line.quoted().as_str().to_owned()),
            Anchor::File(_) => None,
        })
        .collect();
    assert_eq!(quoted, ["given"]);

    // A resolved thread no longer counts as a duplicate.
    store.set_resolved(first.added[0].id(), true).await.unwrap();
    let again = store
        .import_threads(first_batch.clone(), &snapshot_answers(&first_batch))
        .await
        .unwrap();
    assert_eq!(again.added.len(), 1);

    // An unquotable range adds nothing from the batch.
    let bad = vec![request(1, 1, "new", None), request(8, 8, "x", None)];
    let error = store
        .import_threads(bad.clone(), &snapshot_answers(&bad))
        .await
        .unwrap_err();
    assert!(
        matches!(error, CommentError::Unquotable(Unquotable::Range { .. })),
        "{error}"
    );
    assert!(!first_bodies_of(&store).await.contains(&"new".to_owned()));
}

#[tokio::test]
async fn imports_a_payload_without_a_line_as_a_thread_on_the_whole_file_when_the_review_has_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let file = |name: &str, text: &str| NewThread::new(path(name), ThreadTarget::File, body(text));
    let ok = vec![file("a.py", "Split this module.")];
    let outcome = store
        .import_threads(ok.clone(), &snapshot_answers(&ok))
        .await
        .unwrap();
    assert_eq!(outcome.added[0].anchor(), &file_anchor());
    assert!(!outcome.added[0].is_stale());
    let gone = vec![file("gone.py", "x")];
    let error = store
        .import_threads(gone.clone(), &snapshot_answers(&gone))
        .await
        .unwrap_err();
    assert!(
        matches!(error, CommentError::Unquotable(Unquotable::MissingFile(_))),
        "{error}"
    );
}

#[tokio::test]
async fn checks_a_payloads_own_quoted_range_against_the_snapshot_before_keeping_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(&dir, "working").await;
    let target = ThreadTarget::Lines {
        side: Side::New,
        range: range(1, 1),
        quoted: Some(Quote::new("root:x:0:0")),
    };
    let escape = vec![NewThread::new(path("../../etc/passwd"), target, body("x"))];
    let error = store
        .import_threads(escape.clone(), &snapshot_answers(&escape))
        .await
        .unwrap_err();
    assert!(matches!(error, CommentError::Unquotable(_)), "{error}");
    let honest = vec![request(1, 1, "x", Some("as shown"))];
    let outcome = store
        .import_threads(honest.clone(), &snapshot_answers(&honest))
        .await
        .unwrap();
    let Anchor::Line(kept) = outcome.added[0].anchor() else {
        panic!("a line thread")
    };
    assert_eq!(kept.quoted().as_str(), "as shown");
    assert_eq!(
        QuoteQuestion {
            path: path("a.py"),
            side: Side::New,
            range: range(1, 1)
        },
        AnchorQuestions::of(&honest).quotes()[0]
    );
}
