//! The JSON of viewed marks: `ViewedEntrySchema`, and the bulk request around a list of them.

use serde::{Deserialize, Serialize};

use super::viewed_marks::{Blob, ViewedEntry};
use crate::domains::comments::anchors::review_path::{EmptyPath, ReviewPath};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewedEntryWire {
    pub path: String,
    pub blob: String,
    pub viewed: bool,
}

/// `ViewedBulkRequestSchema`: `PUT /api/viewed/bulk`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewedBulkWire {
    pub entries: Vec<ViewedEntryWire>,
}

impl From<&ViewedEntry> for ViewedEntryWire {
    fn from(entry: &ViewedEntry) -> Self {
        Self {
            path: entry.path().as_str().to_owned(),
            blob: entry.blob().as_str().to_owned(),
            viewed: entry.viewed(),
        }
    }
}

impl TryFrom<ViewedEntryWire> for ViewedEntry {
    type Error = EmptyPath;

    fn try_from(wire: ViewedEntryWire) -> Result<Self, Self::Error> {
        Ok(Self::new(
            ReviewPath::parse(wire.path)?,
            Blob::new(wire.blob),
            wire.viewed,
        ))
    }
}

impl TryFrom<ViewedBulkWire> for Vec<ViewedEntry> {
    type Error = EmptyPath;

    fn try_from(wire: ViewedBulkWire) -> Result<Self, Self::Error> {
        wire.entries
            .into_iter()
            .map(ViewedEntry::try_from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_entry_round_trips_through_its_json() {
        let wire = json!({"path": "a.py", "blob": "sha", "viewed": true});
        let entry: ViewedEntryWire = serde_json::from_value(wire.clone()).unwrap();
        let domain = ViewedEntry::try_from(entry).unwrap();
        assert_eq!(
            serde_json::to_value(ViewedEntryWire::from(&domain)).unwrap(),
            wire
        );
    }

    #[test]
    fn a_bulk_request_converts_every_entry_or_none() {
        let bulk = |path: &str| ViewedBulkWire {
            entries: vec![ViewedEntryWire {
                path: path.into(),
                blob: "s".into(),
                viewed: false,
            }],
        };
        assert_eq!(Vec::<ViewedEntry>::try_from(bulk("a.py")).unwrap().len(), 1);
        assert_eq!(Vec::<ViewedEntry>::try_from(bulk("")), Err(EmptyPath));
        let parsed: ViewedBulkWire = serde_json::from_value(json!({"entries": []})).unwrap();
        assert!(Vec::<ViewedEntry>::try_from(parsed).unwrap().is_empty());
    }
}
