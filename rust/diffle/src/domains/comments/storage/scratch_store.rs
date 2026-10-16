//! Helpers for the store tests: a temporary git dir, stores on it, and small builders.

use std::fs as stdfs;

use serde_json::Value;
use tempfile::TempDir;

use super::comment_store::CommentStore;
use super::storage_key::StorageKey;
use crate::domains::comments::anchors::anchor::{Anchor, Quote};
use crate::domains::comments::anchors::lines::LineRange;
use crate::domains::comments::imports::anchor_answers::{AnchorAnswers, AnchorQuestions};
use crate::domains::comments::imports::new_thread::{NewThread, ThreadTarget};
use crate::domains::comments::relocation::reviewed_files::{ReviewedFiles, SideView};
use crate::domains::comments::sample_threads::{line_anchor, path, range};
use crate::domains::comments::threads::body::Body;
use crate::domains::comments::threads::thread::CommentThread;
use crate::domains::comments::threads::thread_query::ThreadQuery;
use crate::domains::comments::viewed::viewed_marks::{Blob, ViewedEntry};
use crate::kernel::side::Side;

pub(crate) fn key(name: &str) -> StorageKey {
    StorageKey::new(name)
}

pub(crate) async fn open(dir: &TempDir, name: &str) -> CommentStore {
    CommentStore::open(dir.path(), &key(name)).await.unwrap()
}

pub(crate) fn body(text: &str) -> Body {
    Body::parse(text).unwrap()
}

pub(crate) fn anchor() -> Anchor {
    Anchor::Line(line_anchor("a.py", Side::New, 2, "b"))
}

pub(crate) fn file_anchor() -> Anchor {
    Anchor::File(path("a.py"))
}

pub(crate) fn comments_json(dir: &TempDir) -> Value {
    let text = stdfs::read_to_string(dir.path().join("diffle/comments.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

pub(crate) fn first_bodies(raw: &Value, key: &str) -> Vec<String> {
    raw["sets"][key]["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["messages"][0]["body"].as_str().unwrap().to_owned())
        .collect()
}

pub(crate) async fn all_threads(store: &CommentStore) -> Vec<CommentThread> {
    store.threads(&ThreadQuery::all()).await.unwrap()
}

pub(crate) async fn first_bodies_of(store: &CommentStore) -> Vec<String> {
    let mut bodies: Vec<String> = all_threads(store)
        .await
        .iter()
        .map(|t| t.messages().first().text().as_str().to_owned())
        .collect();
    bodies.sort();
    bodies
}

pub(crate) fn entry(file: &str, blob: &str, viewed: bool) -> ViewedEntry {
    ViewedEntry::new(path(file), Blob::new(blob), viewed)
}

/// A review where every file exists and each side reads as `contents`, shown as `shown`.
pub(crate) async fn review_of(
    store: &CommentStore,
    contents: &str,
    shown: Option<Vec<LineRange>>,
) -> ReviewedFiles {
    let needs = store.relocation_needs().await.unwrap();
    let mut reviewed = ReviewedFiles::new();
    for (file, side) in needs.sides() {
        let view = match &shown {
            Some(shown) => SideView::hunks(contents, shown.clone()),
            None => SideView::whole(contents),
        };
        reviewed.record_side(file.clone(), *side, Some(view));
    }
    for file in needs.files() {
        reviewed.record_file(file.clone(), true);
    }
    reviewed
}

/// Quotes `a.py` up to line 3 as `L<start>-<end>`; everything else is unquotable.
pub(crate) fn snapshot_answers(requests: &[NewThread]) -> AnchorAnswers {
    let questions = AnchorQuestions::of(requests);
    let mut answers = AnchorAnswers::new();
    for question in questions.quotes() {
        if question.path.as_str() == "a.py" && question.range.end().get() <= 3 {
            let text = format!("L{}-{}", question.range.start(), question.range.end());
            answers.record_quote(question.clone(), Quote::new(text));
        }
    }
    for file in questions
        .files()
        .iter()
        .filter(|file| file.as_str() == "a.py")
    {
        answers.record_file(file.clone());
    }
    answers
}

pub(crate) fn request(start: u64, end: u64, text: &str, quoted: Option<&str>) -> NewThread {
    let target = ThreadTarget::Lines {
        side: Side::New,
        range: range(start, end),
        quoted: quoted.map(Quote::new),
    };
    NewThread::new(path("a.py"), target, body(text))
}
