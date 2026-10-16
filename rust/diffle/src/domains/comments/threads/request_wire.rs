//! The JSON of requests about threads: reply, message patch, resolved, and the thread query.

use serde::{Deserialize, Serialize};

use super::body::{Body, BodyError, MessageText};
use super::thread_query::{ThreadQuery, ThreadState};

/// `ReplyCreateSchema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyWire {
    pub body: String,
}

/// `MessagePatchSchema`: any text, as the TypeScript server accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagePatchWire {
    pub body: String,
}

/// `ResolvedRequestSchema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRequestWire {
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadStateWire {
    Open,
    Resolved,
    All,
}

/// `ThreadQuerySchema`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ThreadQueryWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<ThreadStateWire>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl TryFrom<ReplyWire> for Body {
    type Error = BodyError;

    fn try_from(wire: ReplyWire) -> Result<Self, Self::Error> {
        Self::parse(wire.body)
    }
}

impl From<MessagePatchWire> for MessageText {
    fn from(wire: MessagePatchWire) -> Self {
        Self::new(wire.body)
    }
}

impl From<ThreadStateWire> for ThreadState {
    fn from(state: ThreadStateWire) -> Self {
        match state {
            ThreadStateWire::Open => Self::Open,
            ThreadStateWire::Resolved => Self::Resolved,
            ThreadStateWire::All => Self::All,
        }
    }
}

/// An absent state reads as every thread.
impl From<ThreadQueryWire> for ThreadQuery {
    fn from(wire: ThreadQueryWire) -> Self {
        Self::new(wire.state.map_or(ThreadState::All, Into::into), wire.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reply_needs_a_body_with_something_in_it() {
        let blank: ReplyWire = serde_json::from_value(json!({"body": "  "})).unwrap();
        assert_eq!(Body::try_from(blank), Err(BodyError::Blank));
        let reply: ReplyWire = serde_json::from_value(json!({"body": "fixed"})).unwrap();
        assert_eq!(Body::try_from(reply).unwrap().as_str(), "fixed");
    }

    #[test]
    fn a_patch_may_blank_a_message() {
        let patch: MessagePatchWire = serde_json::from_value(json!({"body": ""})).unwrap();
        assert_eq!(MessageText::from(patch).as_str(), "");
    }

    #[test]
    fn the_resolved_request_reads_one_flag() {
        let request: ResolvedRequestWire =
            serde_json::from_value(json!({"resolved": true})).unwrap();
        assert!(request.resolved);
        assert!(serde_json::from_value::<ResolvedRequestWire>(json!({})).is_err());
    }

    #[test]
    fn a_query_defaults_to_every_thread_and_reads_state_and_path() {
        let empty: ThreadQueryWire = serde_json::from_value(json!({})).unwrap();
        assert_eq!(ThreadQuery::from(empty), ThreadQuery::all());
        let wire: ThreadQueryWire =
            serde_json::from_value(json!({"state": "resolved", "path": "a.py"})).unwrap();
        assert_eq!(
            ThreadQuery::from(wire),
            ThreadQuery::new(ThreadState::Resolved, Some("a.py".into()))
        );
        assert!(serde_json::from_value::<ThreadQueryWire>(json!({"state": "done"})).is_err());
    }
}
