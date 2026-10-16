//! The review as relocation sees it, handed over as data.
//!
//! Relocation runs under the store's file lock, where it must not wait on the repository. So
//! the caller first asks the store what it needs ([`RelocationNeeds`]), reads those files, and
//! passes the answers in ([`ReviewedFiles`]). A thread added meanwhile, whose file nobody read,
//! is left alone until the next refresh.

use std::collections::{BTreeSet, HashMap};

use super::quoting::relocate;
use super::shown_ranges::is_shown;
use crate::domains::comments::anchors::anchor::{Anchor, LineAnchor};
use crate::domains::comments::anchors::lines::LineRange;
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::threads::thread::{CommentThread, Placement};
use crate::kernel::side::Side;

/// One side of a file as the review presents it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideView {
    contents: String,
    shown: Option<Vec<LineRange>>,
}

impl SideView {
    /// A side shown in full.
    #[must_use]
    pub fn whole(contents: impl Into<String>) -> Self {
        Self {
            contents: contents.into(),
            shown: None,
        }
    }

    /// A side of which a diff's hunks show only `shown`.
    #[must_use]
    pub fn hunks(contents: impl Into<String>, shown: Vec<LineRange>) -> Self {
        Self {
            contents: contents.into(),
            shown: Some(shown),
        }
    }

    #[must_use]
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// Whether `range` lies on display.
    #[must_use]
    pub fn shows(&self, range: LineRange) -> bool {
        self.shown
            .as_deref()
            .is_none_or(|shown| is_shown(shown, range))
    }
}

/// The files and sides the threads on disk point at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelocationNeeds {
    sides: BTreeSet<(ReviewPath, Side)>,
    files: BTreeSet<ReviewPath>,
}

impl RelocationNeeds {
    #[must_use]
    pub fn of(threads: &[CommentThread]) -> Self {
        let mut needs = Self::default();
        for thread in threads {
            match thread.anchor() {
                Anchor::Line(line) => {
                    needs.sides.insert((line.path().clone(), line.side()));
                }
                Anchor::File(path) => {
                    needs.files.insert(path.clone());
                }
            }
        }
        needs
    }

    /// The `(path, side)` pairs whose contents (and shown ranges) are needed.
    pub fn sides(&self) -> impl Iterator<Item = &(ReviewPath, Side)> {
        self.sides.iter()
    }

    /// The paths of file threads, for which only presence in the review matters.
    pub fn files(&self) -> impl Iterator<Item = &ReviewPath> {
        self.files.iter()
    }
}

/// What the caller found out. A side with no answer was not asked about.
#[derive(Debug, Clone, Default)]
pub struct ReviewedFiles {
    sides: HashMap<(ReviewPath, Side), Option<SideView>>,
    files: HashMap<ReviewPath, bool>,
}

impl ReviewedFiles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one side of a path; `None` when the review has no such side.
    pub fn record_side(&mut self, path: ReviewPath, side: Side, view: Option<SideView>) {
        self.sides.insert((path, side), view);
    }

    /// Records whether the review has a file at `path` on either side.
    pub fn record_file(&mut self, path: ReviewPath, present: bool) {
        self.files.insert(path, present);
    }

    /// Where the thread's anchor is in this review, or `None` when nobody looked.
    ///
    /// A line thread is lost when its side is gone, its text is not found, or the relocated
    /// range falls outside what the review shows; a file thread when its file left the review.
    #[must_use]
    pub fn placement(&self, thread: &CommentThread) -> Option<Placement> {
        match thread.anchor() {
            Anchor::File(path) => {
                let present = *self.files.get(path)?;
                Some(if present {
                    Placement::Found(thread.anchor().clone())
                } else {
                    Placement::Lost
                })
            }
            Anchor::Line(line) => {
                let view = self.sides.get(&(line.path().clone(), line.side()))?;
                Some(place_line(line, view.as_ref()))
            }
        }
    }
}

fn place_line(anchor: &LineAnchor, view: Option<&SideView>) -> Placement {
    let moved = view.and_then(|view| {
        relocate(anchor, view.contents()).filter(|moved| view.shows(moved.range()))
    });
    moved.map_or(Placement::Lost, |moved| {
        Placement::Found(Anchor::Line(moved))
    })
}

#[cfg(test)]
#[path = "reviewed_files_tests.rs"]
mod tests;
