//! A request that repeats an open thread is skipped, not added twice.

use super::new_thread::{NewThread, ThreadTarget};
use crate::domains::comments::anchors::anchor::Anchor;
use crate::domains::comments::threads::body::trimmed;
use crate::domains::comments::threads::thread::CommentThread;

/// True when `request` duplicates an open thread at the same anchor whose opening message has
/// the same body, ignoring surrounding whitespace.
#[must_use]
pub fn is_duplicate(existing: &[CommentThread], request: &NewThread) -> bool {
    let body = trimmed(request.body().as_str());
    let mut same_words = existing.iter().filter(|thread| {
        !thread.is_resolved()
            && thread.anchor().path() == request.path()
            && trimmed(thread.messages().first().text().as_str()) == body
    });
    match request.target() {
        ThreadTarget::File => same_words.any(|thread| matches!(thread.anchor(), Anchor::File(_))),
        ThreadTarget::Lines { side, range, .. } => same_words.any(|thread| {
            matches!(thread.anchor(), Anchor::Line(line) if line.side() == *side && line.range() == *range)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::{line_anchor, path, range, thread};
    use crate::domains::comments::threads::body::Body;
    use crate::kernel::epoch_millis::EpochMillis;
    use crate::kernel::side::Side;

    fn line_thread(id: &str, start: u64, end: u64, body: &str) -> CommentThread {
        let rows = (start..=end).map(|_| "q").collect::<Vec<_>>().join("\n");
        thread(
            id,
            Anchor::Line(line_anchor("a.py", Side::New, start, &rows)),
            body,
            0,
        )
    }

    fn existing() -> Vec<CommentThread> {
        let mut resolved = line_thread("2", 8, 8, "Resolved finding");
        resolved.set_resolved(true, EpochMillis::new(1));
        vec![line_thread("1", 3, 4, "Same finding "), resolved]
    }

    fn lines(file: &str, side: Side, start: u64, end: u64, body: &str) -> NewThread {
        let target = ThreadTarget::Lines {
            side,
            range: range(start, end),
            quoted: None,
        };
        NewThread::new(path(file), target, Body::parse(body).unwrap())
    }

    fn file(file: &str, body: &str) -> NewThread {
        NewThread::new(path(file), ThreadTarget::File, Body::parse(body).unwrap())
    }

    #[test]
    fn a_file_comment_matches_by_path_and_body_never_against_a_line_thread() {
        let mut threads = existing();
        threads.push(thread("3", Anchor::File(path("a.py")), "Whole file", 0));
        assert!(is_duplicate(&threads, &file("a.py", " Whole file")));
        assert!(!is_duplicate(&threads, &file("b.py", "Whole file")));
        assert!(!is_duplicate(&threads, &file("a.py", "Same finding")));
        assert!(!is_duplicate(
            &threads,
            &lines("a.py", Side::New, 1, 1, "Whole file")
        ));
    }

    #[test]
    fn a_line_comment_matches_path_side_range_and_trimmed_opening_body_on_open_threads_only() {
        let threads = existing();
        assert!(is_duplicate(
            &threads,
            &lines("a.py", Side::New, 3, 4, "Same finding")
        ));
        assert!(!is_duplicate(
            &threads,
            &lines("a.py", Side::New, 3, 4, "Other finding")
        ));
        assert!(!is_duplicate(
            &threads,
            &lines("a.py", Side::New, 3, 3, "Same finding")
        ));
        assert!(!is_duplicate(
            &threads,
            &lines("a.py", Side::Old, 3, 4, "Same finding")
        ));
        assert!(!is_duplicate(
            &threads,
            &lines("a.py", Side::New, 8, 8, "Resolved finding")
        ));
    }
}
