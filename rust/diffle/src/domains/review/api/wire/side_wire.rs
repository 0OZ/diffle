//! `SideSchema`: `"old"` or `"new"`.

use serde::Deserialize;

use crate::kernel::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideWire {
    Old,
    New,
}

impl From<SideWire> for Side {
    fn from(wire: SideWire) -> Self {
        match wire {
            SideWire::Old => Self::Old,
            SideWire::New => Self::New,
        }
    }
}
