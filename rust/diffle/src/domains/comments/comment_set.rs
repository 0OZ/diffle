//! The threads and viewed marks of one comparison, and every rule for changing them.
//!
//! Pure: the store loads a set, applies one of these changes under its file lock, and saves it.

use super::anchors::anchor::Anchor;
use super::comment_error::{NotFound, Unquotable};
use super::imports::anchor_answers::AnchorAnswers;
use super::imports::duplicates::is_duplicate;
use super::imports::new_thread::NewThread;
use super::relocation::reviewed_files::{RelocationNeeds, ReviewedFiles};
use super::threads::body::{Body, MessageText};
use super::threads::ids::{MessageId, ThreadId};
use super::threads::thread::{AfterRemoval, CommentThread, compare_threads};
use super::threads::thread_query::ThreadQuery;
use super::viewed::viewed_marks::{ViewedEntry, ViewedMarks};
use crate::kernel::epoch_millis::EpochMillis;

/// What an import did: the threads it opened, and how many requests repeated an open thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    pub added: Vec<CommentThread>,
    pub skipped: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentSet {
    threads: Vec<CommentThread>,
    viewed: ViewedMarks,
}

impl CommentSet {
    #[must_use]
    pub fn new(threads: Vec<CommentThread>, viewed: ViewedMarks) -> Self {
        Self { threads, viewed }
    }

    #[must_use]
    pub fn threads(&self) -> &[CommentThread] {
        &self.threads
    }

    #[must_use]
    pub fn viewed(&self) -> &ViewedMarks {
        &self.viewed
    }

    /// Threads matching `query`, ordered by path, line, then creation.
    #[must_use]
    pub fn matching(&self, query: &ThreadQuery) -> Vec<CommentThread> {
        let mut found: Vec<CommentThread> = self
            .threads
            .iter()
            .filter(|thread| query.admits(thread))
            .cloned()
            .collect();
        found.sort_by(compare_threads);
        found
    }

    #[must_use]
    pub fn find(&self, id: &ThreadId) -> Option<&CommentThread> {
        self.threads.iter().find(|thread| thread.id() == id)
    }

    pub fn add_thread(&mut self, anchor: Anchor, body: Body, now: EpochMillis) -> CommentThread {
        let thread = CommentThread::open(anchor, body, now);
        self.threads.push(thread.clone());
        thread
    }

    pub fn reply(
        &mut self,
        id: &ThreadId,
        body: Body,
        now: EpochMillis,
    ) -> Result<CommentThread, NotFound> {
        let thread = self.thread_mut(id)?;
        thread.reply(body, now);
        Ok(thread.clone())
    }

    pub fn edit_message(
        &mut self,
        id: &ThreadId,
        message: &MessageId,
        text: MessageText,
        now: EpochMillis,
    ) -> Result<CommentThread, NotFound> {
        let thread = self.thread_mut(id)?;
        thread.edit_message(message, text, now)?;
        Ok(thread.clone())
    }

    /// Removes the message; removes the thread when its last message goes (returns `None` then).
    pub fn remove_message(
        &mut self,
        id: &ThreadId,
        message: &MessageId,
    ) -> Result<Option<CommentThread>, NotFound> {
        let thread = self.thread_mut(id)?;
        match thread.remove_message(message)? {
            AfterRemoval::Remains => Ok(Some(thread.clone())),
            AfterRemoval::Emptied => {
                self.threads.retain(|thread| thread.id() != id);
                Ok(None)
            }
        }
    }

    pub fn set_resolved(
        &mut self,
        id: &ThreadId,
        resolved: bool,
        now: EpochMillis,
    ) -> Result<CommentThread, NotFound> {
        let thread = self.thread_mut(id)?;
        thread.set_resolved(resolved, now);
        Ok(thread.clone())
    }

    pub fn remove_thread(&mut self, id: &ThreadId) -> Result<(), NotFound> {
        let before = self.threads.len();
        self.threads.retain(|thread| thread.id() != id);
        if self.threads.len() == before {
            return Err(NotFound::Thread(id.clone()));
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.threads.clear();
    }

    /// Drops every stale thread. Returns how many went.
    pub fn remove_stale(&mut self) -> usize {
        let before = self.threads.len();
        self.threads.retain(|thread| !thread.is_stale());
        before - self.threads.len()
    }

    /// Opens a thread for each request that is not already an open duplicate (later requests of
    /// the batch may duplicate earlier ones). Every anchor is placed through `answers` first, so
    /// when any cannot be placed nothing is added.
    pub fn import(
        &mut self,
        requests: Vec<NewThread>,
        answers: &AnchorAnswers,
        now: EpochMillis,
    ) -> Result<ImportOutcome, Unquotable> {
        let mut seen = self.threads.clone();
        let mut opened = Vec::new();
        let mut skipped = 0;
        for request in requests {
            if is_duplicate(&seen, &request) {
                skipped += 1;
                continue;
            }
            let thread = CommentThread::open(answers.place(&request)?, request.body().clone(), now);
            seen.push(thread.clone());
            opened.push(thread);
        }
        self.threads.extend(opened.iter().cloned());
        Ok(ImportOutcome {
            added: opened,
            skipped,
        })
    }

    pub fn mark_viewed(&mut self, entry: ViewedEntry) -> Vec<ViewedEntry> {
        self.viewed.mark(entry);
        self.viewed.entries().to_vec()
    }

    /// Replaces the marks of many paths at once; their history is dropped.
    pub fn mark_viewed_many(&mut self, entries: Vec<ViewedEntry>) -> Vec<ViewedEntry> {
        self.viewed.replace_paths(entries);
        self.viewed.entries().to_vec()
    }

    #[must_use]
    pub fn relocation_needs(&self) -> RelocationNeeds {
        RelocationNeeds::of(&self.threads)
    }

    /// Re-anchors every thread `reviewed` knows about. Returns true if anything changed.
    pub fn relocate(&mut self, reviewed: &ReviewedFiles) -> bool {
        let mut changed = false;
        for thread in &mut self.threads {
            if let Some(placement) = reviewed.placement(thread) {
                changed |= thread.apply_placement(placement);
            }
        }
        changed
    }

    fn thread_mut(&mut self, id: &ThreadId) -> Result<&mut CommentThread, NotFound> {
        self.threads
            .iter_mut()
            .find(|thread| thread.id() == id)
            .ok_or_else(|| NotFound::Thread(id.clone()))
    }
}

#[cfg(test)]
#[path = "comment_set_tests.rs"]
mod tests;
