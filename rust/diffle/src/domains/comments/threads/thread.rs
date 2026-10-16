//! A conversation on one anchor.

use std::cmp::Ordering;

use super::body::Body;
use super::ids::{MessageId, ThreadId};
use super::message::{Message, MessageRemoval, Messages};
use crate::domains::comments::anchors::anchor::Anchor;
use crate::domains::comments::anchors::lines::Line;
use crate::domains::comments::comment_error::NotFound;
use crate::kernel::epoch_millis::EpochMillis;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Open,
    /// `at` is absent only for a thread stored without the time it was resolved.
    Resolved {
        at: Option<EpochMillis>,
    },
}

/// Whether the thread still points at something the review shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Fresh,
    /// Relocation failed after a snapshot refresh. `from_line` is where a line thread used to
    /// start; a file thread has none.
    Stale {
        from_line: Option<Line>,
    },
}

/// Where relocation found a thread's anchor in the current review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The anchor as it stands now, possibly moved.
    Found(Anchor),
    Lost,
}

/// What is left of a thread after one of its messages is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterRemoval {
    Remains,
    /// The last message went, so the thread goes with it.
    Emptied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentThread {
    id: ThreadId,
    anchor: Anchor,
    messages: Messages,
    resolution: Resolution,
    freshness: Freshness,
}

impl CommentThread {
    /// A fresh, unresolved thread opened by `body`.
    #[must_use]
    pub fn open(anchor: Anchor, body: Body, now: EpochMillis) -> Self {
        Self::restored(
            ThreadId::generate(),
            anchor,
            Messages::opened_by(Message::written(body, now)),
            Resolution::Open,
            Freshness::Fresh,
        )
    }

    /// A thread as stored.
    #[must_use]
    pub fn restored(
        id: ThreadId,
        anchor: Anchor,
        messages: Messages,
        resolution: Resolution,
        freshness: Freshness,
    ) -> Self {
        Self {
            id,
            anchor,
            messages,
            resolution,
            freshness,
        }
    }

    #[must_use]
    pub fn id(&self) -> &ThreadId {
        &self.id
    }

    #[must_use]
    pub fn anchor(&self) -> &Anchor {
        &self.anchor
    }

    #[must_use]
    pub fn messages(&self) -> &Messages {
        &self.messages
    }

    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    #[must_use]
    pub fn freshness(&self) -> Freshness {
        self.freshness
    }

    #[must_use]
    pub fn is_resolved(&self) -> bool {
        match self.resolution {
            Resolution::Open => false,
            Resolution::Resolved { .. } => true,
        }
    }

    #[must_use]
    pub fn is_stale(&self) -> bool {
        match self.freshness {
            Freshness::Fresh => false,
            Freshness::Stale { .. } => true,
        }
    }

    /// The thread as it reads with only `message` in it, for exporting one message of a
    /// conversation; `None` when the thread has no such message.
    #[must_use]
    pub fn excerpt(&self, message: &MessageId) -> Option<Self> {
        let kept = self.messages.iter().find(|m| m.id() == message)?.clone();
        Some(Self {
            messages: Messages::opened_by(kept),
            ..self.clone()
        })
    }

    pub fn reply(&mut self, body: Body, now: EpochMillis) {
        self.messages.push(Message::written(body, now));
    }

    pub fn edit_message(
        &mut self,
        message: &MessageId,
        text: super::body::MessageText,
        now: EpochMillis,
    ) -> Result<(), NotFound> {
        self.messages.edit(message, text, now)
    }

    pub fn remove_message(&mut self, message: &MessageId) -> Result<AfterRemoval, NotFound> {
        match self.messages.without(message)? {
            MessageRemoval::Kept(rest) => {
                self.messages = rest;
                Ok(AfterRemoval::Remains)
            }
            MessageRemoval::Emptied => Ok(AfterRemoval::Emptied),
        }
    }

    pub fn set_resolved(&mut self, resolved: bool, now: EpochMillis) {
        self.resolution = if resolved {
            Resolution::Resolved { at: Some(now) }
        } else {
            Resolution::Open
        };
    }

    /// Re-anchors the thread; returns whether anything changed.
    pub fn apply_placement(&mut self, placement: Placement) -> bool {
        match placement {
            Placement::Lost => match self.freshness {
                Freshness::Stale { .. } => false,
                Freshness::Fresh => {
                    self.freshness = Freshness::Stale {
                        from_line: self.anchor.start_line(),
                    };
                    true
                }
            },
            Placement::Found(anchor) => {
                let was_stale = self.is_stale();
                self.freshness = Freshness::Fresh;
                let moved = anchor != self.anchor;
                self.anchor = anchor;
                was_stale || moved
            }
        }
    }
}

/// Review order: by path, then line (file threads first), then when the thread was opened.
#[must_use]
pub fn compare_threads(a: &CommentThread, b: &CommentThread) -> Ordering {
    a.anchor
        .path()
        .cmp(b.anchor.path())
        .then_with(|| a.anchor.sort_line().cmp(&b.anchor.sort_line()))
        .then_with(|| {
            a.messages
                .first()
                .created_at()
                .cmp(&b.messages.first().created_at())
        })
}

#[cfg(test)]
#[path = "thread_tests.rs"]
mod tests;
