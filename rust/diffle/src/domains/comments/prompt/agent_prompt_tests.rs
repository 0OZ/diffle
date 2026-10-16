use super::*;
use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::anchors::lines::LineRange;
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::sample_threads::{line, message, range};
use crate::domains::comments::threads::ids::ThreadId;
use crate::domains::comments::threads::message::Messages;
use crate::domains::comments::threads::thread::Resolution;

struct Spec<'a> {
    path: &'a str,
    side: Side,
    range: LineRange,
    quoted: &'a str,
    bodies: &'a [&'a str],
    stale_from: Option<u64>,
}

impl Default for Spec<'_> {
    fn default() -> Self {
        Self {
            path: "src/a.py",
            side: Side::New,
            range: range(3, 3),
            quoted: "def foo():",
            bodies: &["Rename this."],
            stale_from: None,
        }
    }
}

fn messages(bodies: &[&str]) -> Messages {
    let all = bodies
        .iter()
        .enumerate()
        .map(|(i, body)| message("m", body, i as u64 + 1))
        .collect();
    Messages::from_vec(all).unwrap()
}

fn freshness(stale: bool, from: Option<u64>) -> Freshness {
    if stale {
        Freshness::Stale {
            from_line: from.map(line),
        }
    } else {
        Freshness::Fresh
    }
}

fn line_thread(spec: &Spec, stale: bool) -> CommentThread {
    let anchor = LineAnchor::new(
        ReviewPath::parse(spec.path).unwrap(),
        spec.side,
        spec.range,
        Quote::new(spec.quoted),
    );
    CommentThread::restored(
        ThreadId::from("x".to_owned()),
        Anchor::Line(anchor),
        messages(spec.bodies),
        Resolution::Open,
        freshness(stale, spec.stale_from),
    )
}

fn file_thread(bodies: &[&str], stale: bool) -> CommentThread {
    CommentThread::restored(
        ThreadId::from("x".to_owned()),
        Anchor::File(ReviewPath::parse("src/a.py").unwrap()),
        messages(bodies),
        Resolution::Open,
        freshness(stale, None),
    )
}

#[test]
fn renders_ranges_multi_line_quotes_removed_side_and_stale_prefixes() {
    let removed = Spec {
        side: Side::Old,
        range: range(3, 4),
        quoted: "a\n\nb",
        ..Spec::default()
    };
    let stale = Spec {
        path: "src/b.py",
        stale_from: Some(9),
        ..Spec::default()
    };
    let out = format_prompt(&[line_thread(&removed, false), line_thread(&stale, true)]);
    assert_eq!(
        out,
        "(removed) src/a.py:3-4\n\n> a\n>\n> b\n\nRename this.\n\n---\n\
         \n(stale, was line 9) src/b.py:3\n\n> def foo():\n\nRename this.\n\n---\n"
    );
}

#[test]
fn a_stale_line_thread_without_a_recorded_line_was_where_it_is_now() {
    let spec = Spec {
        range: range(7, 7),
        ..Spec::default()
    };
    let out = format_prompt(&[line_thread(&spec, true)]);
    assert!(out.starts_with("(stale, was line 7) src/a.py:7\n"), "{out}");
    let both = Spec {
        side: Side::Old,
        ..spec
    };
    assert!(
        format_prompt(&[line_thread(&both, true)])
            .starts_with("(removed) (stale, was line 7) src/a.py:7\n")
    );
}

#[test]
fn renders_a_thread_on_the_whole_file_with_a_file_prefix_and_no_quote_its_fences_as_written() {
    let file = file_thread(&["Split this module.", "```suggestion\nx\n```"], false);
    assert_eq!(
        format_prompt(&[file]),
        "(file) src/a.py\n\nSplit this module.\n\n```suggestion\nx\n```\n\n---\n"
    );
    let out = format_prompt(&[file_thread(&["Split."], true)]);
    assert!(out.starts_with("(file) (stale) src/a.py\n"), "{out}");
}

#[test]
fn a_suggestion_expands_to_original_and_suggested_fences_around_prose() {
    let spec = Spec {
        bodies: &["Before.\n```suggestion\nnew()\n```\nAfter."],
        ..Spec::default()
    };
    assert_eq!(
        format_prompt(&[line_thread(&spec, false)]),
        "src/a.py:3\n\n> def foo():\n\nBefore.\n\nORIGINAL:\n```\ndef foo():\n```\nSUGGESTED:\n```\nnew()\n```\n\nAfter.\n\n---\n"
    );
}

#[test]
fn threads_come_out_in_review_order_and_no_threads_in_no_text() {
    let second = Spec {
        path: "b.py",
        ..Spec::default()
    };
    let first = Spec {
        path: "a.py",
        ..Spec::default()
    };
    let out = format_prompt(&[line_thread(&second, false), line_thread(&first, false)]);
    assert!(out.starts_with("a.py:3"), "{out}");
    assert_eq!(format_prompt(&[]), "");
}

// One realistic export, kept as a file so a change to the format is judged by reading the
// prompt an agent would receive rather than a string literal.
#[test]
fn a_review_reads_as_an_agent_would_receive_it() {
    let sale = "A line is a sale unless its ``kind`` column says ``refund``; refunds count negative.\n\"\"\"";
    let symbols = "_SYMBOLS = {\"EUR\": \"€\", \"USD\": \"$\", \"GBP\": \"£\", \"JPY\": \"¥\"}";
    let legacy = "def read_tsv(path: Path) -> Ledger:\n    ledger = Ledger()\n    for line in path.read_text().splitlines():\n        if not line or line.startswith(\"#\"):\n            continue\n        account, description, quantity, unit_price = line.split(\"\\t\")\n        ledger.add(Entry(account, description, int(quantity), Decimal(unit_price)))";
    let threads = [
        line_thread(
            &Spec {
                path: "tally/ledger.py",
                range: range(3, 4),
                quoted: sale,
                bodies: &[
                    "Say what counts as a refund here, not only in the parser.",
                    "Also link the release notes once they mention `kind`.",
                ],
                ..Spec::default()
            },
            false,
        ),
        line_thread(
            &Spec {
                path: "tally/currency.py",
                range: range(7, 7),
                quoted: symbols,
                bodies: &[
                    "`KWD` gets three decimals above but no symbol here.\n\n```suggestion\n_SYMBOLS = {\"EUR\": \"€\", \"USD\": \"$\", \"GBP\": \"£\", \"JPY\": \"¥\", \"KWD\": \"KD\"}\n```",
                ],
                ..Spec::default()
            },
            false,
        ),
        line_thread(
            &Spec {
                path: "tally/legacy.py",
                side: Side::Old,
                range: range(9, 15),
                quoted: legacy,
                bodies: &["Is the archive really converted? The cron job still calls this."],
                ..Spec::default()
            },
            false,
        ),
        file_thread_at(
            "tally/refunds.py",
            "Consider folding this into `ledger.py`; it only has two callers.",
        ),
        line_thread(
            &Spec {
                path: "tally/cli.py",
                range: range(27, 27),
                quoted: "print(\"no entries\", file=sys.stderr)",
                bodies: &[
                    "Exit code 1 for \"nothing to do\" will trip `set -e` in the cron wrapper.",
                ],
                stale_from: Some(26),
                ..Spec::default()
            },
            true,
        ),
    ];
    assert_eq!(format_prompt(&threads), include_str!("fixtures/prompt.md"));
}

fn file_thread_at(file: &str, body: &str) -> CommentThread {
    CommentThread::restored(
        ThreadId::from("x".to_owned()),
        Anchor::File(ReviewPath::parse(file).unwrap()),
        messages(&[body]),
        Resolution::Open,
        Freshness::Fresh,
    )
}
