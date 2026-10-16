//! The shapes `comments.json` has had, and how an older one becomes the current.
//!
//! - v1: flat comments, one anchor each.
//! - v2: threads whose anchors are line ranges without a `kind`.
//! - v3 (current): threads over line or file anchors.
//!
//! Only the set a store owns is read into types; the other keys' sets pass through as JSON, so
//! one store never drops what another wrote.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::domains::comments::anchors::anchor_wire::{AnchorWire, LineAnchorWire};
use crate::domains::comments::anchors::review_path::EmptyPath;
use crate::domains::comments::comment_set::CommentSet;
use crate::domains::comments::threads::ids::MessageId;
use crate::domains::comments::threads::thread::CommentThread;
use crate::domains::comments::threads::thread_wire::{
    CommentThreadWire, MessageWire, ThreadWire, ThreadWireError,
};
use crate::domains::comments::viewed::viewed_marks::{ViewedEntry, ViewedMarks};
use crate::domains::comments::viewed::viewed_wire::ViewedEntryWire;

const CURRENT_VERSION: u64 = 3;
const OLDEST_VERSION: u64 = 1;

/// A stored set that breaks a rule the domain keeps.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoredDataError {
    #[error(transparent)]
    Thread(#[from] ThreadWireError),
    #[error(transparent)]
    Viewed(#[from] EmptyPath),
}

/// One key's set, as stored in version 3.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetStore {
    #[serde(default)]
    pub threads: Vec<CommentThreadWire>,
    #[serde(default)]
    pub viewed: Vec<ViewedEntryWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V1Comment {
    id: String,
    anchor: LineAnchorWire,
    body: String,
    created_at: u64,
    updated_at: u64,
    #[serde(default)]
    stale: bool,
    #[serde(default)]
    stale_from_line: Option<u64>,
}

#[derive(Deserialize, Default)]
struct V1Set {
    #[serde(default)]
    comments: Vec<V1Comment>,
    #[serde(default)]
    viewed: Vec<ViewedEntryWire>,
}

#[derive(Deserialize, Default)]
struct V2Set {
    #[serde(default)]
    threads: Vec<ThreadWire<LineAnchorWire>>,
    #[serde(default)]
    viewed: Vec<ViewedEntryWire>,
}

#[derive(Serialize)]
struct FileOut<'a> {
    version: u64,
    sets: &'a Map<String, Value>,
}

/// The file as stored: its version and every key's set.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredFile {
    version: u64,
    sets: Map<String, Value>,
}

impl StoredFile {
    /// What an absent file reads as.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            version: CURRENT_VERSION,
            sets: Map::new(),
        }
    }

    /// Unknown shapes read as an empty store; malformed JSON is an error.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        let value: Value = serde_json::from_str(text)?;
        let version = value.get("version").and_then(Value::as_u64);
        let sets = value.get("sets").and_then(Value::as_object);
        Ok(match (version, sets) {
            (Some(version @ OLDEST_VERSION..=CURRENT_VERSION), Some(sets)) => Self {
                version,
                sets: sets.clone(),
            },
            _ => Self::empty(),
        })
    }

    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        self.version == CURRENT_VERSION
    }

    /// Any stored format as the current one.
    pub fn upgrade(self) -> Result<Self, serde_json::Error> {
        let version = self.version;
        if version == CURRENT_VERSION {
            return Ok(self);
        }
        let mut sets = Map::new();
        for (key, set) in self.sets {
            let upgraded = if version == OLDEST_VERSION {
                SetStore::from(serde_json::from_value::<V1Set>(set)?)
            } else {
                SetStore::from(serde_json::from_value::<V2Set>(set)?)
            };
            sets.insert(key, serde_json::to_value(upgraded)?);
        }
        Ok(Self {
            version: CURRENT_VERSION,
            sets,
        })
    }

    /// The set filed under `key`; empty when there is none.
    pub fn set(&self, key: &str) -> Result<SetStore, serde_json::Error> {
        self.sets.get(key).map_or_else(
            || Ok(SetStore::default()),
            |set| serde_json::from_value(set.clone()),
        )
    }

    /// The same file with `key`'s set replaced; every other key is untouched.
    pub fn with_set(mut self, key: &str, set: &SetStore) -> Result<Self, serde_json::Error> {
        self.sets.insert(key.to_owned(), serde_json::to_value(set)?);
        Ok(self)
    }

    /// The text to write, indented like the TypeScript server writes it.
    pub fn render(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&FileOut {
            version: self.version,
            sets: &self.sets,
        })
    }
}

impl From<V1Set> for SetStore {
    /// Each v1 comment becomes a single-message, unresolved thread.
    fn from(v1: V1Set) -> Self {
        let threads = v1
            .comments
            .into_iter()
            .map(|comment| CommentThreadWire {
                id: comment.id,
                anchor: AnchorWire::Line(comment.anchor),
                messages: vec![MessageWire {
                    id: MessageId::generate().to_string(),
                    body: comment.body,
                    created_at: comment.created_at,
                    updated_at: comment.updated_at,
                }],
                resolved: false,
                resolved_at: None,
                stale: comment.stale,
                stale_from_line: comment.stale_from_line,
            })
            .collect();
        Self {
            threads,
            viewed: v1.viewed,
        }
    }
}

impl From<V2Set> for SetStore {
    /// Every v2 anchor is a line range; v3 says so.
    fn from(v2: V2Set) -> Self {
        Self {
            threads: v2
                .threads
                .into_iter()
                .map(|thread| thread.map_anchor(AnchorWire::Line))
                .collect(),
            viewed: v2.viewed,
        }
    }
}

impl From<&CommentSet> for SetStore {
    fn from(set: &CommentSet) -> Self {
        Self {
            threads: set.threads().iter().map(CommentThreadWire::from).collect(),
            viewed: set
                .viewed()
                .entries()
                .iter()
                .map(ViewedEntryWire::from)
                .collect(),
        }
    }
}

impl TryFrom<SetStore> for CommentSet {
    type Error = StoredDataError;

    fn try_from(stored: SetStore) -> Result<Self, Self::Error> {
        let threads = stored
            .threads
            .into_iter()
            .map(CommentThread::try_from)
            .collect::<Result<_, _>>()?;
        let viewed = stored
            .viewed
            .into_iter()
            .map(ViewedEntry::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self::new(threads, ViewedMarks::new(viewed)))
    }
}

#[cfg(test)]
#[path = "versions_store_tests.rs"]
mod tests;
