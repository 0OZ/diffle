use super::*;

fn parse(args: &[&str]) -> Result<Revspec, RevspecError> {
    let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    Revspec::parse(&owned)
}

fn shown(args: &[&str]) -> String {
    parse(args).unwrap().to_string()
}

#[test]
fn each_form_reads_like_the_git_diff_it_stands_for() {
    assert_eq!(shown(&["main"]), "main...HEAD");
    assert_eq!(shown(&["a..b"]), "a..b");
    assert_eq!(shown(&["a...b"]), "a...b");
    assert_eq!(shown(&["a", "b"]), "a..b");
    assert_eq!(shown(&["a^!"]), "a^!");
    assert_eq!(shown(&["worktree"]), "HEAD..worktree");
}

#[test]
fn empty_sides_are_head() {
    assert_eq!(shown(&["main.."]), "main..HEAD");
    assert_eq!(shown(&["..main"]), "HEAD..main");
    assert_eq!(shown(&["...main"]), "HEAD...main");
    assert_eq!(shown(&["^!"]), "HEAD^!");
}

#[test]
fn the_worktree_is_accepted_on_either_side() {
    assert_eq!(shown(&["main..worktree"]), "main..worktree");
    assert_eq!(shown(&["worktree..main"]), "worktree..main");
    assert_eq!(shown(&["main...worktree"]), "main...worktree");
    assert_eq!(shown(&["worktree", "HEAD"]), "worktree..HEAD");
    assert!(parse(&["main..worktree"]).unwrap().touches_worktree());
    assert!(!parse(&["main"]).unwrap().touches_worktree());
}

#[test]
fn the_worktree_is_not_a_commit() {
    assert_eq!(
        parse(&["worktree^!"]),
        Err(RevspecError::WorktreeIsNotACommit)
    );
}

#[test]
fn rejects_what_cannot_be_combined() {
    assert_eq!(parse(&[]), Err(RevspecError::ArgCount));
    assert_eq!(parse(&["a", "b", "c"]), Err(RevspecError::ArgCount));
    assert_eq!(
        parse(&["a..b", "c"]),
        Err(RevspecError::RangeWithTwoRevisions)
    );
    assert_eq!(
        parse(&["a", "b^!"]),
        Err(RevspecError::CommitWithTwoRevisions)
    );
    assert_eq!(parse(&["a..b^!"]), Err(RevspecError::CommitWithRange));
}

#[test]
fn rejects_names_git_would_read_as_options() {
    assert!(matches!(
        parse(&["--output=x"]),
        Err(RevspecError::Invalid { .. })
    ));
    assert!(matches!(
        parse(&["a", ""]),
        Err(RevspecError::Invalid { .. })
    ));
}

#[test]
fn names_the_revisions_but_not_the_worktree() {
    let named = |args: &[&str]| {
        let owned: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        Revspec::parse(&owned)
            .unwrap()
            .named_revisions()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(named(&["main...feat"]), ["main", "feat"]);
    assert_eq!(named(&["main..worktree"]), ["main"]);
    assert_eq!(named(&["worktree"]), ["HEAD"]);
    assert_eq!(named(&["abc1234^!"]), ["abc1234"]);
}

#[test]
fn only_a_range_ending_at_the_worktree_has_uncommitted_changes() {
    for (args, ends) in [
        (["main..worktree"], true),
        (["main...worktree"], true),
        (["worktree..main"], false),
        (["main..feat"], false),
        (["abc1234^!"], false),
    ] {
        assert_eq!(parse(&args).unwrap().ends_at_worktree(), ends, "{args:?}");
    }
}
