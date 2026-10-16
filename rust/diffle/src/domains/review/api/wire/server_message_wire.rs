//! `ServerMessageSchema`: what the server pushes down `/ws`.

use serde::Serialize;

use crate::domains::review::session::session_events::SessionEvent;
use crate::domains::review::snapshot::review_snapshot::Moved;

/// `MovedSchema`: the compared refs resolve elsewhere than when the shown snapshot was taken.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovedWire {
    /// The snapshot a reload would supersede.
    version: u64,
    old_sha: String,
    new_sha: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerMessageWire {
    Snapshot {
        version: u64,
    },
    /// `null` once the refs are back where the snapshot has them.
    Moved {
        moved: Option<MovedWire>,
    },
    Threads,
    Viewed,
    Config,
}

impl From<&Moved> for MovedWire {
    fn from(moved: &Moved) -> Self {
        Self {
            version: moved.version.get(),
            old_sha: moved.old.to_string(),
            new_sha: moved.new.to_string(),
        }
    }
}

impl From<&SessionEvent> for ServerMessageWire {
    fn from(event: &SessionEvent) -> Self {
        match event {
            SessionEvent::Snapshot(version) => Self::Snapshot {
                version: version.get(),
            },
            SessionEvent::Moved(moved) => Self::Moved {
                moved: moved.as_ref().map(MovedWire::from),
            },
            SessionEvent::Threads => Self::Threads,
            SessionEvent::Viewed => Self::Viewed,
            SessionEvent::Config => Self::Config,
        }
    }
}
