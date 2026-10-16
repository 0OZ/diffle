use super::*;
use crate::domains::comments::sample_threads::{line, line_anchor, path, range, thread};

fn on_line(start: u64, quoted: &str) -> CommentThread {
    thread(
        "t",
        Anchor::Line(line_anchor("a.py", Side::New, start, quoted)),
        "x",
        1,
    )
}

fn reviewing(view: Option<SideView>) -> ReviewedFiles {
    let mut reviewed = ReviewedFiles::new();
    reviewed.record_side(path("a.py"), Side::New, view);
    reviewed
}

#[test]
fn needs_name_each_side_and_file_once() {
    let threads = [
        on_line(2, "b"),
        on_line(5, "e"),
        thread("f", Anchor::File(path("a.py")), "x", 1),
        thread("g", Anchor::File(path("a.py")), "x", 1),
    ];
    let needs = RelocationNeeds::of(&threads);
    assert_eq!(
        needs.sides().collect::<Vec<_>>(),
        [&(path("a.py"), Side::New)]
    );
    assert_eq!(needs.files().collect::<Vec<_>>(), [&path("a.py")]);
}

#[test]
fn a_line_follows_its_text_through_a_side_shown_whole() {
    let reviewed = reviewing(Some(SideView::whole("z\na\nb\n")));
    let Some(Placement::Found(Anchor::Line(moved))) = reviewed.placement(&on_line(2, "b")) else {
        panic!("the text is still there");
    };
    assert_eq!(moved.range().start(), line(3));
}

#[test]
fn a_line_is_lost_when_its_side_or_text_is_gone() {
    assert_eq!(
        reviewing(None).placement(&on_line(2, "b")),
        Some(Placement::Lost)
    );
    let gone = reviewing(Some(SideView::whole("nothing here\n")));
    assert_eq!(gone.placement(&on_line(2, "b")), Some(Placement::Lost));
}

#[test]
fn a_line_whose_text_survives_outside_the_shown_ranges_is_lost() {
    let contents = "a\nb\nc\nd\ne\n";
    let thread = on_line(2, "b");
    let outside = reviewing(Some(SideView::hunks(contents, vec![range(4, 5)])));
    assert_eq!(outside.placement(&thread), Some(Placement::Lost));
    let inside = reviewing(Some(SideView::hunks(contents, vec![range(1, 3)])));
    assert!(matches!(
        inside.placement(&thread),
        Some(Placement::Found(_))
    ));
    let nothing = reviewing(Some(SideView::hunks(contents, Vec::new())));
    assert_eq!(nothing.placement(&thread), Some(Placement::Lost));
}

#[test]
fn a_file_thread_follows_its_file_whatever_the_contents_say() {
    let file = thread("f", Anchor::File(path("a.py")), "x", 1);
    let mut reviewed = ReviewedFiles::new();
    reviewed.record_file(path("a.py"), true);
    assert_eq!(
        reviewed.placement(&file),
        Some(Placement::Found(file.anchor().clone()))
    );
    reviewed.record_file(path("a.py"), false);
    assert_eq!(reviewed.placement(&file), Some(Placement::Lost));
}

#[test]
fn a_thread_nobody_looked_up_has_no_placement() {
    let reviewed = ReviewedFiles::new();
    assert_eq!(reviewed.placement(&on_line(2, "b")), None);
    assert_eq!(
        reviewed.placement(&thread("f", Anchor::File(path("a.py")), "x", 1)),
        None
    );
}

#[test]
fn a_side_view_shows_what_its_hunks_cover() {
    let whole = SideView::whole("a\nb\n");
    assert_eq!(whole.contents(), "a\nb\n");
    assert!(whole.shows(range(1, 2)));
    let hunks = SideView::hunks("a\nb\nc\n", vec![range(2, 3)]);
    assert!(hunks.shows(range(2, 3)));
    assert!(!hunks.shows(range(1, 2)));
}
