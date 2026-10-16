use super::*;
use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::imports::anchor_answers::QuoteQuestion;
use crate::domains::comments::imports::new_thread::ThreadTarget;
use crate::domains::comments::relocation::reviewed_files::SideView;
use crate::domains::comments::sample_threads::{line_anchor, path, range};
use crate::domains::comments::viewed::viewed_marks::Blob;
use crate::kernel::side::Side;

fn now(millis: u64) -> EpochMillis {
    EpochMillis::new(millis)
}

fn anchor(start: u64, quoted: &str) -> Anchor {
    Anchor::Line(line_anchor("a.py", Side::New, start, quoted))
}

fn body(text: &str) -> Body {
    Body::parse(text).unwrap()
}

fn texts(thread: &CommentThread) -> Vec<&str> {
    thread
        .messages()
        .iter()
        .map(|m| m.text().as_str())
        .collect()
}

fn request(start: u64, end: u64, text: &str, quoted: Option<&str>) -> NewThread {
    let target = ThreadTarget::Lines {
        side: Side::New,
        range: range(start, end),
        quoted: quoted.map(Quote::new),
    };
    NewThread::new(path("a.py"), target, body(text))
}

/// Answers every range up to line 3 of `a.py` with `L<start>-<end>`, and knows only `a.py`.
fn answers_for(requests: &[NewThread]) -> AnchorAnswers {
    use crate::domains::comments::imports::anchor_answers::AnchorQuestions;
    let questions = AnchorQuestions::of(requests);
    let mut answers = AnchorAnswers::new();
    for QuoteQuestion {
        path: file,
        side,
        range,
    } in questions.quotes().iter().cloned()
    {
        if file.as_str() == "a.py" && range.end().get() <= 3 {
            let text = format!("L{}-{}", range.start(), range.end());
            answers.record_quote(
                QuoteQuestion {
                    path: file,
                    side,
                    range,
                },
                Quote::new(text),
            );
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

#[test]
fn replies_edits_removes_messages_and_threads_resolves_and_clears() {
    let mut set = CommentSet::default();
    let id = set
        .add_thread(anchor(2, "b"), body("hello"), now(1))
        .id()
        .clone();
    set.reply(&id, body("fixed"), now(2)).unwrap();
    assert_eq!(texts(set.find(&id).unwrap()), ["hello", "fixed"]);
    let first = set.find(&id).unwrap().messages().first().id().clone();
    let second = set
        .find(&id)
        .unwrap()
        .messages()
        .iter()
        .nth(1)
        .unwrap()
        .id()
        .clone();
    set.edit_message(&id, &first, MessageText::new("hello again"), now(3))
        .unwrap();
    assert_eq!(texts(set.find(&id).unwrap())[0], "hello again");
    assert!(set.remove_message(&id, &second).unwrap().is_some());
    set.set_resolved(&id, true, now(4)).unwrap();
    assert!(
        set.matching(&ThreadQuery::new(
            crate::domains::comments::threads::thread_query::ThreadState::Open,
            None
        ))
        .is_empty()
    );
    assert!(set.find(&id).unwrap().is_resolved());
    set.set_resolved(&id, false, now(5)).unwrap();
    assert!(!set.find(&id).unwrap().is_resolved());
    // The last message takes the thread with it.
    assert_eq!(set.remove_message(&id, &first), Ok(None));
    assert!(set.threads().is_empty());
    assert_eq!(set.remove_thread(&id), Err(NotFound::Thread(id.clone())));
    assert_eq!(
        set.reply(&id, body("x"), now(6)),
        Err(NotFound::Thread(id.clone()))
    );
    set.add_thread(anchor(2, "b"), body("hello"), now(7));
    set.clear();
    assert!(set.threads().is_empty());
}

#[test]
fn a_thread_is_removed_by_id_and_a_missing_message_is_not_found() {
    let mut set = CommentSet::default();
    let id = set
        .add_thread(anchor(2, "b"), body("hello"), now(1))
        .id()
        .clone();
    let missing = MessageId::from("nope".to_owned());
    assert_eq!(
        set.remove_message(&id, &missing),
        Err(NotFound::Message(missing.clone()))
    );
    assert_eq!(
        set.edit_message(&id, &missing, MessageText::new("x"), now(2)),
        Err(NotFound::Message(missing))
    );
    set.remove_thread(&id).unwrap();
    assert!(set.threads().is_empty());
}

#[test]
fn matching_orders_by_path_line_then_creation_and_filters_by_path() {
    let mut set = CommentSet::default();
    let z = Anchor::Line(line_anchor("z.py", Side::New, 1, "z"));
    set.add_thread(z, body("z"), now(1));
    set.add_thread(anchor(2, "b"), body("a"), now(2));
    let all: Vec<String> = set
        .matching(&ThreadQuery::all())
        .iter()
        .map(|t| t.anchor().path().to_string())
        .collect();
    assert_eq!(all, ["a.py", "z.py"]);
    let only_z = ThreadQuery::new(Default::default(), Some("z.py".into()));
    assert_eq!(set.matching(&only_z).len(), 1);
}

#[test]
fn imports_quote_from_the_snapshot_and_skip_open_duplicates() {
    let mut set = CommentSet::default();
    let first = vec![request(1, 1, "one", None)];
    let outcome = set
        .import(first.clone(), &answers_for(&first), now(1))
        .unwrap();
    assert_eq!(outcome.skipped, 0);
    let Anchor::Line(placed) = outcome.added[0].anchor() else {
        panic!("a line thread")
    };
    assert_eq!(placed.quoted().as_str(), "L1-1");
    assert_eq!(texts(&outcome.added[0]), ["one"]);

    let batch = vec![
        request(1, 1, "one", None),
        request(2, 3, "two", Some("given")),
        request(2, 3, "two", None),
    ];
    let second = set
        .import(batch.clone(), &answers_for(&batch), now(2))
        .unwrap();
    assert_eq!(second.skipped, 2);
    let quoted: Vec<&str> = second
        .added
        .iter()
        .filter_map(|t| match t.anchor() {
            Anchor::Line(line) => Some(line.quoted().as_str()),
            Anchor::File(_) => None,
        })
        .collect();
    assert_eq!(quoted, ["given"]);

    // A resolved thread no longer counts as a duplicate.
    set.set_resolved(outcome.added[0].id(), true, now(3))
        .unwrap();
    let again = set
        .import(first.clone(), &answers_for(&first), now(4))
        .unwrap();
    assert_eq!(again.added.len(), 1);
}

#[test]
fn an_unquotable_range_adds_nothing_from_the_batch() {
    let mut set = CommentSet::default();
    let batch = vec![request(1, 1, "new", None), request(8, 8, "x", None)];
    let error = set
        .import(batch.clone(), &answers_for(&batch), now(1))
        .unwrap_err();
    assert!(matches!(error, Unquotable::Range { .. }));
    assert!(set.threads().is_empty());
}

#[test]
fn a_duplicate_is_skipped_even_when_its_lines_are_no_longer_quotable() {
    let mut set = CommentSet::default();
    let kept = vec![request(1, 1, "one", None)];
    set.import(kept.clone(), &answers_for(&kept), now(1))
        .unwrap();
    let outcome = set.import(kept, &AnchorAnswers::new(), now(2)).unwrap();
    assert_eq!((outcome.added.len(), outcome.skipped), (0, 1));
}

#[test]
fn a_file_import_needs_the_file_in_the_review() {
    let mut set = CommentSet::default();
    let ok = NewThread::new(path("a.py"), ThreadTarget::File, body("Split this module."));
    let gone = NewThread::new(path("gone.py"), ThreadTarget::File, body("x"));
    let outcome = set
        .import(vec![ok.clone()], &answers_for(&[ok]), now(1))
        .unwrap();
    assert_eq!(outcome.added[0].anchor(), &Anchor::File(path("a.py")));
    assert!(!outcome.added[0].is_stale());
    let error = set
        .import(vec![gone.clone()], &answers_for(&[gone]), now(2))
        .unwrap_err();
    assert_eq!(error, Unquotable::MissingFile(path("gone.py")));
}

#[test]
fn removing_stale_threads_leaves_the_rest() {
    let mut set = CommentSet::default();
    let gone = set
        .add_thread(anchor(2, "missing"), body("x"), now(1))
        .id()
        .clone();
    let kept = set
        .add_thread(anchor(2, "b"), body("x"), now(2))
        .id()
        .clone();
    let mut reviewed = ReviewedFiles::new();
    reviewed.record_side(path("a.py"), Side::New, Some(SideView::whole("a\nb\n")));
    assert!(set.relocate(&reviewed));
    assert!(set.find(&gone).unwrap().is_stale());
    assert_eq!(set.remove_stale(), 1);
    assert_eq!(set.remove_stale(), 0);
    assert_eq!(
        set.threads()
            .iter()
            .map(|t| t.id().clone())
            .collect::<Vec<_>>(),
        [kept]
    );
}

#[test]
fn relocation_flags_stale_follows_moves_and_leaves_unasked_threads_alone() {
    let mut set = CommentSet::default();
    let id = set
        .add_thread(anchor(2, "b"), body("hello"), now(1))
        .id()
        .clone();
    let reviewing = |contents: &str| {
        let mut reviewed = ReviewedFiles::new();
        reviewed.record_side(path("a.py"), Side::New, Some(SideView::whole(contents)));
        reviewed
    };
    assert_eq!(set.relocation_needs().sides().count(), 1);
    assert!(set.relocate(&reviewing("z\na\nb\n")));
    assert_eq!(
        set.find(&id).unwrap().anchor().start_line().unwrap().get(),
        3
    );
    assert!(set.relocate(&reviewing("nothing here\n")));
    assert!(set.find(&id).unwrap().is_stale());
    assert!(set.relocate(&reviewing("b\n")));
    assert!(!set.find(&id).unwrap().is_stale());
    assert!(!set.relocate(&reviewing("b\n")));
    assert!(!set.relocate(&ReviewedFiles::new()));
}

#[test]
fn viewed_marks_are_kept_on_the_set() {
    let mut set = CommentSet::default();
    let entry = |blob: &str| ViewedEntry::new(path("a.py"), Blob::new(blob), true);
    assert_eq!(set.mark_viewed(entry("sha1")), [entry("sha1")]);
    assert_eq!(set.mark_viewed_many(vec![entry("sha2")]), [entry("sha2")]);
    assert_eq!(set.viewed().entries(), [entry("sha2")]);
}
