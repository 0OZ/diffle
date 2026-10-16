//! The JSON of a thread: `CommentThreadSchema`, which is also how `comments.json` stores one.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::body::MessageText;
use super::ids::{MessageId, ThreadId};
use super::message::{Message, Messages};
use super::thread::{CommentThread, Freshness, Resolution};
use crate::domains::comments::anchors::anchor::Anchor;
use crate::domains::comments::anchors::anchor_wire::{AnchorWire, AnchorWireError};
use crate::domains::comments::anchors::lines::{Line, ZeroLine};
use crate::kernel::epoch_millis::EpochMillis;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageWire {
    pub id: String,
    pub body: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// A thread over any anchor shape: version 3 files use [`AnchorWire`], version 2 files line
/// anchors without a `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadWire<A> {
    pub id: String,
    pub anchor: A,
    pub messages: Vec<MessageWire>,
    pub resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<u64>,
    pub stale: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_from_line: Option<u64>,
}

pub type CommentThreadWire = ThreadWire<AnchorWire>;

/// A stored thread that breaks a rule the domain keeps.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ThreadWireError {
    #[error(transparent)]
    Anchor(#[from] AnchorWireError),
    #[error("thread {0} has no messages")]
    NoMessages(String),
    #[error(transparent)]
    StaleLine(#[from] ZeroLine),
}

impl<A> ThreadWire<A> {
    /// The same thread over another anchor shape.
    pub fn map_anchor<B>(self, convert: impl FnOnce(A) -> B) -> ThreadWire<B> {
        ThreadWire {
            id: self.id,
            anchor: convert(self.anchor),
            messages: self.messages,
            resolved: self.resolved,
            resolved_at: self.resolved_at,
            stale: self.stale,
            stale_from_line: self.stale_from_line,
        }
    }
}

impl From<&Message> for MessageWire {
    fn from(message: &Message) -> Self {
        Self {
            id: message.id().to_string(),
            body: message.text().as_str().to_owned(),
            created_at: message.created_at().get(),
            updated_at: message.updated_at().get(),
        }
    }
}

impl From<MessageWire> for Message {
    fn from(wire: MessageWire) -> Self {
        Self::restored(
            MessageId::from(wire.id),
            MessageText::new(wire.body),
            EpochMillis::new(wire.created_at),
            EpochMillis::new(wire.updated_at),
        )
    }
}

impl From<&CommentThread> for CommentThreadWire {
    fn from(thread: &CommentThread) -> Self {
        let (resolved, resolved_at) = match thread.resolution() {
            Resolution::Open => (false, None),
            Resolution::Resolved { at } => (true, at.map(EpochMillis::get)),
        };
        let (stale, stale_from_line) = match thread.freshness() {
            Freshness::Fresh => (false, None),
            Freshness::Stale { from_line } => (true, from_line.map(Line::get)),
        };
        Self {
            id: thread.id().to_string(),
            anchor: AnchorWire::from(thread.anchor()),
            messages: thread.messages().iter().map(MessageWire::from).collect(),
            resolved,
            resolved_at,
            stale,
            stale_from_line,
        }
    }
}

impl TryFrom<CommentThreadWire> for CommentThread {
    type Error = ThreadWireError;

    fn try_from(wire: CommentThreadWire) -> Result<Self, Self::Error> {
        let messages = Messages::from_vec(wire.messages.into_iter().map(Message::from).collect())
            .ok_or_else(|| ThreadWireError::NoMessages(wire.id.clone()))?;
        let resolution = if wire.resolved {
            Resolution::Resolved {
                at: wire.resolved_at.map(EpochMillis::new),
            }
        } else {
            Resolution::Open
        };
        let freshness = if wire.stale {
            Freshness::Stale {
                from_line: wire.stale_from_line.map(Line::new).transpose()?,
            }
        } else {
            Freshness::Fresh
        };
        Ok(Self::restored(
            ThreadId::from(wire.id),
            Anchor::try_from(wire.anchor)?,
            messages,
            resolution,
            freshness,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn stored() -> Value {
        json!({
            "id": "t1",
            "anchor": {"kind":"line","path":"a.py","side":"new","startLine":2,"endLine":2,"quoted":"b"},
            "messages": [{"id":"m1","body":"note","createdAt":5,"updatedAt":6}],
            "resolved": true,
            "resolvedAt": 7,
            "stale": true,
            "staleFromLine": 9
        })
    }

    #[test]
    fn a_thread_round_trips_through_its_json() {
        let wire: CommentThreadWire = serde_json::from_value(stored()).unwrap();
        let thread = CommentThread::try_from(wire).unwrap();
        assert!(thread.is_resolved() && thread.is_stale());
        assert_eq!(
            serde_json::to_value(CommentThreadWire::from(&thread)).unwrap(),
            stored()
        );
    }

    #[test]
    fn optional_fields_are_omitted_when_absent() {
        let mut json = stored();
        let fields = json.as_object_mut().unwrap();
        fields.insert("resolved".into(), json!(false));
        fields.insert("stale".into(), json!(false));
        fields.remove("resolvedAt");
        fields.remove("staleFromLine");
        let wire: CommentThreadWire = serde_json::from_value(json.clone()).unwrap();
        let thread = CommentThread::try_from(wire).unwrap();
        assert_eq!(
            serde_json::to_value(CommentThreadWire::from(&thread)).unwrap(),
            json
        );
    }

    #[test]
    fn a_stored_thread_without_messages_or_with_a_zero_stale_line_is_refused() {
        let mut json = stored();
        json["messages"] = json!([]);
        let wire: CommentThreadWire = serde_json::from_value(json).unwrap();
        assert_eq!(
            CommentThread::try_from(wire),
            Err(ThreadWireError::NoMessages("t1".into()))
        );
        let mut json = stored();
        json["staleFromLine"] = json!(0);
        let wire: CommentThreadWire = serde_json::from_value(json).unwrap();
        assert!(matches!(
            CommentThread::try_from(wire),
            Err(ThreadWireError::StaleLine(_))
        ));
    }

    #[test]
    fn mapping_the_anchor_keeps_the_rest_of_the_thread() {
        let wire: CommentThreadWire = serde_json::from_value(stored()).unwrap();
        let mapped = wire.clone().map_anchor(|_| 1_u8);
        assert_eq!(
            (mapped.id.as_str(), mapped.anchor, mapped.stale_from_line),
            ("t1", 1, Some(9))
        );
        let message = MessageWire::from(&Message::from(wire.messages[0].clone()));
        assert_eq!(message, wire.messages[0]);
    }
}
