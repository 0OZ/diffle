//! Viewed marks: a path is viewed (or not) at one new-side blob.

use crate::domains::comments::anchors::review_path::ReviewPath;

/// Viewed marks kept per path: the current one plus the newest previous, so a re-touched blob
/// can be recognised.
const HISTORY_PER_PATH: usize = 2;

/// A new-side blob id, opaque to this domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Blob(String);

impl Blob {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An explicit viewed/unviewed mark for a path at a specific new-side content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewedEntry {
    path: ReviewPath,
    blob: Blob,
    viewed: bool,
}

impl ViewedEntry {
    #[must_use]
    pub fn new(path: ReviewPath, blob: Blob, viewed: bool) -> Self {
        Self { path, blob, viewed }
    }

    #[must_use]
    pub fn path(&self) -> &ReviewPath {
        &self.path
    }

    #[must_use]
    pub fn blob(&self) -> &Blob {
        &self.blob
    }

    #[must_use]
    pub fn viewed(&self) -> bool {
        self.viewed
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewedMarks(Vec<ViewedEntry>);

impl ViewedMarks {
    #[must_use]
    pub fn new(entries: Vec<ViewedEntry>) -> Self {
        Self(entries)
    }

    #[must_use]
    pub fn entries(&self) -> &[ViewedEntry] {
        &self.0
    }

    /// Marks or unmarks a path at a blob. The newest previous entry for the path (at another
    /// blob) is kept, so the history never grows past [`HISTORY_PER_PATH`].
    pub fn mark(&mut self, entry: ViewedEntry) {
        let (same_path, mut marks): (Vec<_>, Vec<_>) = std::mem::take(&mut self.0)
            .into_iter()
            .partition(|existing| existing.path == entry.path);
        let previous: Vec<_> = same_path
            .into_iter()
            .filter(|existing| existing.blob != entry.blob)
            .collect();
        let keep_from = previous.len().saturating_sub(HISTORY_PER_PATH - 1);
        marks.extend(previous.into_iter().skip(keep_from));
        marks.push(entry);
        self.0 = marks;
    }

    /// Replaces the marks of every path in `entries`; their history is dropped.
    pub fn replace_paths(&mut self, entries: Vec<ViewedEntry>) {
        let mut marks: Vec<_> = std::mem::take(&mut self.0)
            .into_iter()
            .filter(|existing| entries.iter().all(|entry| entry.path != existing.path))
            .collect();
        marks.extend(entries);
        self.0 = marks;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::comments::sample_threads::path;

    fn entry(file: &str, blob: &str, viewed: bool) -> ViewedEntry {
        ViewedEntry::new(path(file), Blob::new(blob), viewed)
    }

    #[test]
    fn keeps_the_newest_previous_mark_per_path_and_caps_the_history() {
        let mut marks = ViewedMarks::default();
        marks.mark(entry("a.py", "sha1", true));
        marks.mark(entry("a.py", "sha1", false));
        assert_eq!(marks.entries(), [entry("a.py", "sha1", false)]);
        marks.mark(entry("a.py", "sha2", true));
        marks.mark(entry("a.py", "sha3", true));
        assert_eq!(
            marks.entries(),
            [entry("a.py", "sha2", true), entry("a.py", "sha3", true)]
        );
    }

    #[test]
    fn marking_one_path_leaves_the_others_where_they_were() {
        let mut marks = ViewedMarks::new(vec![entry("b.py", "x", true), entry("a.py", "y", true)]);
        marks.mark(entry("a.py", "z", false));
        let paths: Vec<_> = marks.entries().iter().map(|e| e.path().as_str()).collect();
        assert_eq!(paths, ["b.py", "a.py", "a.py"]);
        let last = &marks.entries()[2];
        assert_eq!((last.blob().as_str(), last.viewed()), ("z", false));
    }

    #[test]
    fn replacing_paths_drops_their_history() {
        let mut marks =
            ViewedMarks::new(vec![entry("a.py", "s2", true), entry("a.py", "s3", true)]);
        marks.mark(entry("b.py", "x", true));
        marks.replace_paths(vec![entry("a.py", "sha4", false)]);
        assert_eq!(
            marks.entries(),
            [entry("b.py", "x", true), entry("a.py", "sha4", false)]
        );
    }
}
