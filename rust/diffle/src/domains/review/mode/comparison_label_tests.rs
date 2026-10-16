use super::*;
use crate::domains::repository::names::rev_name::RevName;
use crate::domains::revisions::comment_key::CommentKey;
use crate::domains::revisions::comparison::Liveness;

fn comparison(spec: Revspec) -> Comparison {
    Comparison::new(spec, Liveness::Refs, CommentKey::working())
}

fn rev(text: &str) -> Revision {
    Revision::Rev(RevName::parse(text).unwrap())
}

#[test]
fn names_branches_by_their_short_names() {
    let direct = comparison(Revspec::Direct {
        old: rev("refs/remotes/origin/main"),
        new: Revision::Worktree,
    });
    assert_eq!(comparison_label(&direct), "origin/main..worktree");
    let merge_base = comparison(Revspec::MergeBase {
        old: rev("refs/heads/main"),
        new: rev("HEAD"),
    });
    assert_eq!(comparison_label(&merge_base), "main...HEAD");
}

#[test]
fn a_commit_is_its_abbreviation() {
    let full = "0123456789abcdef0123456789abcdef01234567";
    let commit = comparison(Revspec::commit(RevName::parse(full).unwrap()));
    assert_eq!(comparison_label(&commit), "0123456^!");
    let short = comparison(Revspec::commit(RevName::parse("abc").unwrap()));
    assert_eq!(comparison_label(&short), "abc^!");
}
