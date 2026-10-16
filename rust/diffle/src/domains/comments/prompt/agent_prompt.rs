//! The agent prompt: a review's threads, one block each, in review order.

use super::suggestions::{BodySegment, parse_suggestions};
use crate::domains::comments::anchors::anchor::{Anchor, LineAnchor};
use crate::domains::comments::threads::body::trimmed;
use crate::domains::comments::threads::message::Message;
use crate::domains::comments::threads::thread::{CommentThread, Freshness, compare_threads};
use crate::kernel::side::Side;

/// Agent-prompt format, one block per thread:
///
/// ```text
/// path:line            (or path:start-end; `(file) path` for a thread on the whole file)
///
/// > quoted line        (absent for a file thread)
///
/// body                 (a thread's messages joined by a blank line)
///
/// ---
/// ```
///
/// A ```suggestion fence in a body expands to ORIGINAL / SUGGESTED fences; a file thread has no
/// original, so its fences stay as written. Blocks are separated by a blank line.
#[must_use]
pub fn format_prompt(threads: &[CommentThread]) -> String {
    let mut ordered: Vec<&CommentThread> = threads.iter().collect();
    ordered.sort_by(|a, b| compare_threads(a, b));
    ordered
        .into_iter()
        .map(format_one)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_one(thread: &CommentThread) -> String {
    match thread.anchor() {
        Anchor::File(path) => {
            let stale = if thread.is_stale() { "(stale) " } else { "" };
            let body = joined(
                thread
                    .messages()
                    .iter()
                    .map(|m| trimmed(m.text().as_str()).to_owned()),
            );
            format!("(file) {stale}{path}\n\n{body}\n\n---\n")
        }
        Anchor::Line(anchor) => {
            let header = line_header(thread, anchor);
            let quote = quote_block(anchor.quoted().as_str());
            let body = joined(
                thread
                    .messages()
                    .iter()
                    .map(|message| format_message(message, anchor.quoted().as_str())),
            );
            format!("{header}\n\n{quote}\n\n{body}\n\n---\n")
        }
    }
}

fn line_header(thread: &CommentThread, anchor: &LineAnchor) -> String {
    let mut prefixes = Vec::new();
    match anchor.side() {
        Side::Old => prefixes.push("(removed)".to_owned()),
        Side::New => {}
    }
    match thread.freshness() {
        Freshness::Fresh => {}
        Freshness::Stale { from_line } => {
            let was = from_line.unwrap_or(anchor.range().start());
            prefixes.push(format!("(stale, was line {was})"));
        }
    }
    let prefix = prefixes.iter().map(|p| format!("{p} ")).collect::<String>();
    format!("{prefix}{}:{}", anchor.path(), anchor.range())
}

fn quote_block(quoted: &str) -> String {
    quoted
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_message(message: &Message, quoted: &str) -> String {
    let parts: Vec<String> = parse_suggestions(message.text().as_str())
        .into_iter()
        .map(|segment| match segment {
            BodySegment::Prose(text) => trimmed(&text).to_owned(),
            BodySegment::Suggestion(code) => render_suggestion(quoted, &code),
        })
        .filter(|part| !part.is_empty())
        .collect();
    parts.join("\n\n")
}

fn render_suggestion(original: &str, code: &str) -> String {
    format!("ORIGINAL:\n```\n{original}\n```\nSUGGESTED:\n```\n{code}\n```")
}

fn joined(parts: impl Iterator<Item = String>) -> String {
    parts.collect::<Vec<_>>().join("\n\n")
}

#[cfg(test)]
#[path = "agent_prompt_tests.rs"]
mod tests;
