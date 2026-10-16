use super::*;
use crate::domains::comments::sample_threads::{self, line, path};
use crate::domains::comments::threads::body::MessageText;
use crate::kernel::side::Side;

fn line_anchor(file: &str, start: u64, quoted: &str) -> Anchor {
    Anchor::Line(sample_threads::line_anchor(file, Side::New, start, quoted))
}

fn thread_at(id: &str, anchor: Anchor, created_at: u64) -> CommentThread {
    sample_threads::thread(id, anchor, "b", created_at)
}

fn opened(anchor: Anchor) -> CommentThread {
    CommentThread::open(anchor, Body::parse("hello").unwrap(), EpochMillis::new(10))
}

#[test]
fn threads_order_by_path_then_line_with_the_file_thread_first_then_creation() {
    let mut threads = [
        thread_at("z", line_anchor("z.py", 1, "x"), 1),
        thread_at("later-file", Anchor::File(path("a.py")), 5),
        thread_at("line", line_anchor("a.py", 3, "x"), 1),
        thread_at("file", Anchor::File(path("a.py")), 2),
        thread_at("other", Anchor::File(path("b.py")), 1),
    ];
    threads.sort_by(compare_threads);
    let ids: Vec<&str> = threads.iter().map(|t| t.id().as_str()).collect();
    assert_eq!(ids, ["file", "later-file", "line", "other", "z"]);
}

#[test]
fn a_new_thread_is_open_fresh_and_has_one_message() {
    let thread = opened(line_anchor("a.py", 2, "b"));
    assert_eq!(thread.resolution(), Resolution::Open);
    assert_eq!(thread.freshness(), Freshness::Fresh);
    assert!(!thread.is_resolved() && !thread.is_stale());
    assert_eq!(thread.messages().iter().count(), 1);
    assert_eq!(thread.messages().first().created_at(), EpochMillis::new(10));
}

#[test]
fn replying_editing_and_removing_messages() {
    let mut thread = opened(line_anchor("a.py", 2, "b"));
    thread.reply(Body::parse("fixed").unwrap(), EpochMillis::new(11));
    let first = thread.messages().first().id().clone();
    let second = thread.messages().iter().nth(1).unwrap().id().clone();
    thread
        .edit_message(&first, MessageText::new("again"), EpochMillis::new(12))
        .unwrap();
    assert_eq!(thread.messages().first().text().as_str(), "again");
    assert_eq!(thread.remove_message(&second), Ok(AfterRemoval::Remains));
    assert_eq!(thread.remove_message(&first), Ok(AfterRemoval::Emptied));
    assert_eq!(
        thread.remove_message(&second),
        Err(NotFound::Message(second))
    );
}

#[test]
fn resolving_stamps_the_time_and_reopening_clears_it() {
    let mut thread = opened(Anchor::File(path("a.py")));
    thread.set_resolved(true, EpochMillis::new(20));
    assert_eq!(
        thread.resolution(),
        Resolution::Resolved {
            at: Some(EpochMillis::new(20))
        }
    );
    assert!(thread.is_resolved());
    thread.set_resolved(false, EpochMillis::new(21));
    assert_eq!(thread.resolution(), Resolution::Open);
}

#[test]
fn a_lost_placement_marks_the_thread_stale_from_its_line_once() {
    let mut thread = opened(line_anchor("a.py", 3, "b"));
    assert!(thread.apply_placement(Placement::Lost));
    assert_eq!(
        thread.freshness(),
        Freshness::Stale {
            from_line: Some(line(3))
        }
    );
    assert!(!thread.apply_placement(Placement::Lost));
    let mut file = opened(Anchor::File(path("a.py")));
    assert!(file.apply_placement(Placement::Lost));
    assert_eq!(file.freshness(), Freshness::Stale { from_line: None });
}

#[test]
fn a_found_placement_clears_staleness_and_follows_a_move() {
    let mut thread = opened(line_anchor("a.py", 3, "b"));
    let same = thread.anchor().clone();
    assert!(!thread.apply_placement(Placement::Found(same.clone())));
    assert!(thread.apply_placement(Placement::Found(line_anchor("a.py", 1, "b"))));
    assert_eq!(thread.anchor().start_line(), Some(line(1)));
    thread.apply_placement(Placement::Lost);
    assert!(thread.is_stale());
    assert!(thread.apply_placement(Placement::Found(thread.anchor().clone())));
    assert!(!thread.is_stale());
    assert_ne!(thread.anchor(), &same);
}

#[test]
fn an_excerpt_keeps_one_message_and_the_rest_of_the_thread() {
    let mut thread = opened(line_anchor("a.py", 3, "x"));
    thread.reply(
        crate::domains::comments::threads::body::Body::parse("second").unwrap(),
        crate::kernel::epoch_millis::EpochMillis::new(9),
    );
    let reply = thread.messages().iter().nth(1).unwrap().id().clone();
    let excerpt = thread.excerpt(&reply).unwrap();
    assert_eq!(excerpt.id(), thread.id());
    assert_eq!(excerpt.anchor(), thread.anchor());
    assert_eq!(excerpt.messages().iter().count(), 1);
    assert_eq!(excerpt.messages().first().text().as_str(), "second");
    let unknown = crate::domains::comments::threads::ids::MessageId::from("nope".to_owned());
    assert_eq!(thread.excerpt(&unknown), None);
}
