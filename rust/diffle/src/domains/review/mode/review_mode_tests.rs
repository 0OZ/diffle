use super::*;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::revisions::revision::Revision;

fn id(text: &str) -> ObjectId {
    ObjectId::parse(&text.repeat(40)).unwrap()
}

fn range(live: Liveness) -> Comparison {
    let spec = Revspec::Direct {
        old: Revision::Rev(RevName::parse("main").unwrap()),
        new: Revision::head(),
    };
    Comparison::new(spec, live, CommentKey::working())
}

#[test]
fn a_whole_mode_is_its_own_range() {
    let mode = ReviewMode::whole(range(Liveness::Refs));
    assert_eq!(mode.range(), mode.comparison());
    assert_eq!(mode.within(), None);
    assert!(!mode.is_single_commit());
    assert_eq!(mode.unfocused(), mode);
}

#[test]
fn a_focused_commit_is_pinned_but_follows_its_ranges_refs() {
    let commit = id("a");
    let mode = ReviewMode::commit_within(&commit, range(Liveness::Refs));
    assert!(mode.is_single_commit());
    assert_eq!(mode.comparison().live(), Liveness::Refs);
    assert_eq!(
        mode.comparison().comment_key().as_str(),
        format!("commit:{commit}")
    );
    assert_eq!(mode.range().live(), Liveness::Refs);
    assert_eq!(mode.unfocused(), ReviewMode::whole(range(Liveness::Refs)));

    let pinned = ReviewMode::commit_within(&commit, range(Liveness::Pinned));
    assert_eq!(pinned.comparison().live(), Liveness::Pinned);
}

#[test]
fn the_uncommitted_changes_of_a_range_share_the_working_key() {
    let mode = ReviewMode::worktree_within(range(Liveness::Worktree));
    assert_eq!(mode.comparison().spec().to_string(), "HEAD..worktree");
    assert_eq!(mode.comparison().live(), Liveness::Worktree);
    assert_eq!(mode.comparison().comment_key().as_str(), "working");
    assert_eq!(mode.range().spec().to_string(), "main..HEAD");
}
