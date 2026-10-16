//! Small builders for tests: anchors, messages and threads without the ceremony.

use super::anchors::anchor::{Anchor, LineAnchor, Quote};
use super::anchors::lines::{Line, LineRange};
use super::anchors::review_path::ReviewPath;
use super::threads::body::MessageText;
use super::threads::ids::{MessageId, ThreadId};
use super::threads::message::{Message, Messages};
use super::threads::thread::{CommentThread, Freshness, Resolution};
use crate::kernel::epoch_millis::EpochMillis;
use crate::kernel::side::Side;

pub fn line(number: u64) -> Line {
    Line::new(number).unwrap()
}

pub fn path(text: &str) -> ReviewPath {
    ReviewPath::parse(text).unwrap()
}

pub fn range(start: u64, end: u64) -> LineRange {
    LineRange::new(line(start), line(end)).unwrap()
}

/// A line anchor on `file` whose range is as tall as `quoted` has lines.
pub fn line_anchor(file: &str, side: Side, start: u64, quoted: &str) -> LineAnchor {
    let rows = u64::try_from(quoted.split('\n').count()).unwrap();
    LineAnchor::new(
        path(file),
        side,
        range(start, start + rows - 1),
        Quote::new(quoted),
    )
}

pub fn message(id: &str, text: &str, at: u64) -> Message {
    Message::restored(
        MessageId::from(id.to_owned()),
        MessageText::new(text),
        EpochMillis::new(at),
        EpochMillis::new(at),
    )
}

/// An open, fresh thread with one message created at `at`.
pub fn thread(id: &str, anchor: Anchor, text: &str, at: u64) -> CommentThread {
    CommentThread::restored(
        ThreadId::from(id.to_owned()),
        anchor,
        Messages::opened_by(message(&format!("{id}-m"), text, at)),
        Resolution::Open,
        Freshness::Fresh,
    )
}
