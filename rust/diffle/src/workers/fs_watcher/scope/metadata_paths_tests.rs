use super::*;
use std::sync::Arc;

fn worktree(git_dir: &str, common_dir: &str) -> WatchTarget {
    WatchTarget::Worktree {
        root: "/wt".into(),
        git_dir: git_dir.into(),
        common_dir: common_dir.into(),
        ignored: Arc::new(|_| false),
    }
}

fn refs(git_dir: &str, common_dir: &str) -> WatchTarget {
    WatchTarget::Refs {
        git_dir: git_dir.into(),
        common_dir: common_dir.into(),
    }
}

#[test]
fn one_git_dir_is_listed_once() {
    let paths = MetadataPaths::of(&worktree("/r/.git", "/r/.git"));
    assert_eq!(paths.git_dirs(), [PathBuf::from("/r/.git")]);
    assert_eq!(paths.refs_dir(), Path::new("/r/.git/refs"));
}

#[test]
fn a_linked_worktree_listens_to_both_git_dirs() {
    let paths = MetadataPaths::of(&worktree("/r/.git/worktrees/w", "/r/.git"));
    assert_eq!(paths.git_dirs().len(), 2);
    assert!(paths.is_metadata(Path::new("/r/.git/worktrees/w/HEAD")));
    assert!(paths.is_metadata(Path::new("/r/.git/worktrees/w/index")));
    assert!(paths.is_metadata(Path::new("/r/.git/HEAD")));
    assert!(!paths.is_metadata(Path::new("/r/.git/index")));
}

#[test]
fn a_worktree_watches_the_index_and_a_refs_target_does_not() {
    let index = Path::new("/r/.git/index");
    assert!(MetadataPaths::of(&worktree("/r/.git", "/r/.git")).is_metadata(index));
    assert!(!MetadataPaths::of(&refs("/r/.git", "/r/.git")).is_metadata(index));
}

#[test]
fn refs_and_packed_refs_count_and_noise_does_not() {
    let paths = MetadataPaths::of(&refs("/r/.git", "/r/.git"));
    for yes in [
        "/r/.git/HEAD",
        "/r/.git/packed-refs",
        "/r/.git/refs",
        "/r/.git/refs/heads/main",
        "/r/.git/refs/heads/team/topic",
        "/r/.git/refs/tags/v1",
    ] {
        assert!(paths.is_metadata(Path::new(yes)), "{yes}");
    }
    for no in [
        "/r/.git/ORIG_HEAD",
        "/r/.git/FETCH_HEAD",
        "/r/.git/objects/ab/cdef",
        "/r/.git/refsx",
        "/r/.git/HEAD/x",
    ] {
        assert!(!paths.is_metadata(Path::new(no)), "{no}");
    }
}

#[test]
fn inside_git_covers_the_whole_subtree_only() {
    let paths = MetadataPaths::of(&refs("/r/.git", "/r/.git"));
    assert!(paths.is_inside_git(Path::new("/r/.git/objects/x")));
    assert!(paths.is_inside_git(Path::new("/r/.git")));
    assert!(!paths.is_inside_git(Path::new("/r/.github/x")));
    assert!(!paths.is_inside_git(Path::new("/r/src")));
}
