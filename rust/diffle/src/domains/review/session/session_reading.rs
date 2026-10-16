//! What the session lets be read from a snapshot, and how comments are placed against it.
//! Every read goes through the snapshot's allowlist: ignored files, `.git` and traversal read
//! as absent.

use crate::domains::comments::anchors::anchor::Quote;
use crate::domains::comments::anchors::review_path::ReviewPath;
use crate::domains::comments::imports::anchor_answers::{
    AnchorAnswers, AnchorQuestions, QuoteQuestion,
};
use crate::domains::comments::relocation::quoting::quote_range;
use crate::domains::comments::relocation::reviewed_files::{ReviewedFiles, SideView};
use crate::domains::comments::relocation::shown_ranges::shown_ranges;
use crate::domains::repository::names::repo_path::RepoPath;
use crate::domains::repository::process::git_error::GitError;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::review_session::{Active, Session};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::kernel::bounded::map_limit;
use crate::kernel::side::Side;

/// Files read at once while placing comments.
const PLACEMENT_READS: usize = 8;

impl Session {
    /// A file on one side of `snapshot`. `path` is a snapshot path (a changed file's new path
    /// or any tree path); the old side of a rename resolves to its old path here.
    pub async fn read_side(
        &self,
        snapshot: &Snapshot,
        path: &RepoPath,
        side: Side,
    ) -> Result<Option<Vec<u8>>, GitError> {
        match snapshot.side_path(path, side) {
            Some(target) => self.repo.read_file(snapshot.source(side), &target).await,
            None => Ok(None),
        }
    }

    /// Where imported comments land: ranges quoted from the snapshot when the import brought
    /// no text, and whether a file thread's file is in the review.
    pub async fn anchor_answers(
        &self,
        snapshot: &Snapshot,
        questions: &AnchorQuestions,
    ) -> Result<AnchorAnswers, ReviewError> {
        let asked = questions.quotes().to_vec();
        let quoted = map_limit(asked, PLACEMENT_READS, |question| async move {
            let quote = self.quote_of(snapshot, &question).await;
            (question, quote)
        })
        .await;
        let mut answers = AnchorAnswers::new();
        for (question, quote) in quoted {
            if let Some(quote) = quote? {
                answers.record_quote(question, quote);
            }
        }
        for path in questions.files() {
            if in_review(snapshot, path) {
                answers.record_file(path.clone());
            }
        }
        Ok(answers)
    }

    async fn quote_of(
        &self,
        snapshot: &Snapshot,
        question: &QuoteQuestion,
    ) -> Result<Option<Quote>, GitError> {
        let Ok(path) = RepoPath::parse(question.path.as_str()) else {
            return Ok(None);
        };
        let bytes = self.read_side(snapshot, &path, question.side).await?;
        Ok(bytes.and_then(|bytes| quote_range(&String::from_utf8_lossy(&bytes), question.range)))
    }

    /// Re-anchors `active`'s threads against what `snapshot` shows. A changed file shows its
    /// hunks; an unchanged tree file shows its new side whole (the file view) and nothing of its
    /// old side. A file thread only needs its file in the review.
    pub(super) async fn relocate_comments(
        &self,
        active: &Active,
        snapshot: &Snapshot,
    ) -> Result<(), ReviewError> {
        let needs = active.comments.relocation_needs().await?;
        let sides: Vec<_> = needs.sides().cloned().collect();
        let views = map_limit(sides, PLACEMENT_READS, |(path, side)| async move {
            let view = self.side_view(active, snapshot, &path, side).await;
            (path, side, view)
        })
        .await;
        let mut reviewed = ReviewedFiles::new();
        for (path, side, view) in views {
            reviewed.record_side(path, side, view?);
        }
        for path in needs.files() {
            reviewed.record_file(path.clone(), in_review(snapshot, path));
        }
        active.comments.relocate_all(&reviewed).await?;
        Ok(())
    }

    async fn side_view(
        &self,
        active: &Active,
        snapshot: &Snapshot,
        path: &ReviewPath,
        side: Side,
    ) -> Result<Option<SideView>, ReviewError> {
        let Ok(path) = RepoPath::parse(path.as_str()) else {
            return Ok(None);
        };
        let Some(bytes) = self.read_side(snapshot, &path, side).await? else {
            return Ok(None);
        };
        let contents = String::from_utf8_lossy(&bytes).into_owned();
        if snapshot.changed_file(&path).is_none() {
            return Ok(match side {
                Side::New => Some(SideView::whole(contents)),
                Side::Old => None,
            });
        }
        let patch = active.snapshotter.patch(&path).await?.unwrap_or_default();
        Ok(Some(SideView::hunks(contents, shown_ranges(&patch, side))))
    }
}

fn in_review(snapshot: &Snapshot, path: &ReviewPath) -> bool {
    RepoPath::parse(path.as_str()).is_ok_and(|path| snapshot.has_file(&path))
}
