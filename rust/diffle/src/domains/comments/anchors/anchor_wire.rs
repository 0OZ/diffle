//! The JSON shape of an anchor: `{"kind":"line", ...}` or `{"kind":"file", "path"}`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::anchor::{Anchor, LineAnchor, Quote};
use super::lines::{InvertedRange, Line, LineRange, ZeroLine};
use super::review_path::{EmptyPath, ReviewPath};
use crate::kernel::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideWire {
    Old,
    New,
}

/// A line anchor without its `kind`: the whole anchor of a version 2 file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineAnchorWire {
    pub path: String,
    pub side: SideWire,
    pub start_line: u64,
    pub end_line: u64,
    pub quoted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AnchorWire {
    Line(LineAnchorWire),
    File { path: String },
}

/// A stored anchor that breaks a rule the domain keeps.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AnchorWireError {
    #[error(transparent)]
    Path(#[from] EmptyPath),
    #[error(transparent)]
    Line(#[from] ZeroLine),
    #[error(transparent)]
    Range(#[from] InvertedRange),
}

impl From<Side> for SideWire {
    fn from(side: Side) -> Self {
        match side {
            Side::Old => Self::Old,
            Side::New => Self::New,
        }
    }
}

impl From<SideWire> for Side {
    fn from(side: SideWire) -> Self {
        match side {
            SideWire::Old => Self::Old,
            SideWire::New => Self::New,
        }
    }
}

impl From<&LineAnchor> for LineAnchorWire {
    fn from(anchor: &LineAnchor) -> Self {
        Self {
            path: anchor.path().as_str().to_owned(),
            side: anchor.side().into(),
            start_line: anchor.range().start().get(),
            end_line: anchor.range().end().get(),
            quoted: anchor.quoted().as_str().to_owned(),
        }
    }
}

impl TryFrom<LineAnchorWire> for LineAnchor {
    type Error = AnchorWireError;

    fn try_from(wire: LineAnchorWire) -> Result<Self, Self::Error> {
        let range = LineRange::new(Line::new(wire.start_line)?, Line::new(wire.end_line)?)?;
        Ok(Self::new(
            ReviewPath::parse(wire.path)?,
            wire.side.into(),
            range,
            Quote::new(wire.quoted),
        ))
    }
}

impl From<&Anchor> for AnchorWire {
    fn from(anchor: &Anchor) -> Self {
        match anchor {
            Anchor::Line(line) => Self::Line(line.into()),
            Anchor::File(path) => Self::File {
                path: path.as_str().to_owned(),
            },
        }
    }
}

impl TryFrom<AnchorWire> for Anchor {
    type Error = AnchorWireError;

    fn try_from(wire: AnchorWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            AnchorWire::Line(line) => Self::Line(line.try_into()?),
            AnchorWire::File { path } => Self::File(ReviewPath::parse(path)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_line_anchor_is_tagged_and_camel_cased() {
        let wire = json!({"kind":"line","path":"a.py","side":"old","startLine":2,"endLine":3,"quoted":"b\nc"});
        let anchor: AnchorWire = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(&anchor).unwrap(), wire);
        let domain = Anchor::try_from(anchor).unwrap();
        assert!(matches!(&domain, Anchor::Line(line) if line.side() == Side::Old));
        assert_eq!(
            serde_json::to_value(AnchorWire::from(&domain)).unwrap(),
            wire
        );
    }

    #[test]
    fn a_file_anchor_carries_only_its_path() {
        let wire = json!({"kind":"file","path":"a.py"});
        let anchor: AnchorWire = serde_json::from_value(wire.clone()).unwrap();
        let domain = Anchor::try_from(anchor).unwrap();
        assert_eq!(
            serde_json::to_value(AnchorWire::from(&domain)).unwrap(),
            wire
        );
    }

    #[test]
    fn a_stored_anchor_that_breaks_a_rule_is_refused() {
        let line = |path: &str, start, end| LineAnchorWire {
            path: path.into(),
            side: SideWire::New,
            start_line: start,
            end_line: end,
            quoted: String::new(),
        };
        assert_eq!(
            LineAnchor::try_from(line("", 1, 1)),
            Err(AnchorWireError::Path(EmptyPath))
        );
        assert_eq!(
            LineAnchor::try_from(line("a", 0, 1)),
            Err(AnchorWireError::Line(ZeroLine))
        );
        assert_eq!(
            LineAnchor::try_from(line("a", 3, 2)),
            Err(AnchorWireError::Range(InvertedRange))
        );
    }
}
