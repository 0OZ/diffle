//! A path list longer than a command line holds.

use super::*;

/// Far more pathspecs than a command line holds: the search must still run, and still only
/// look where it was told to.
fn paths_beyond_the_command_line(real: &RepoPath) -> Vec<RepoPath> {
    const FAKE_PATHS: usize = 60_000;
    let mut paths: Vec<RepoPath> = (0..FAKE_PATHS)
        .map(|n| {
            path(&format!(
                "generated/module-{n:06}/src/nothing-to-find-here.txt"
            ))
        })
        .collect();
    paths.push(real.clone());
    paths
}

#[tokio::test]
async fn a_path_list_too_long_for_the_command_line_still_searches() {
    let fixture = TestRepo::init();
    fixture.commit("wanted.txt", "needle\n");
    fixture.commit("other.txt", "needle\n");
    let options = GrepOptions {
        paths: Some(paths_beyond_the_command_line(&path("wanted.txt"))),
        ..GrepOptions::default()
    };
    let found = search(&fixture, "needle", &TreeSource::Worktree, &options).await;
    // `other.txt` also matches; it is simply not in the list.
    assert_eq!(hits(&found), [("wanted.txt", 1)]);
}

#[tokio::test]
async fn a_long_path_list_and_a_range_filter_apply_together() {
    let fixture = TestRepo::init();
    fixture.commit("wanted.txt", "needle\nneedle\n");
    let wanted = path("wanted.txt");
    let options = GrepOptions {
        paths: Some(paths_beyond_the_command_line(&wanted)),
        ranges: Some(HashMap::from([(wanted, vec![span(2, 2)])])),
        ..GrepOptions::default()
    };
    let found = search(&fixture, "needle", &TreeSource::Worktree, &options).await;
    assert_eq!(hits(&found), [("wanted.txt", 2)]);
}

#[test]
fn a_list_rides_the_command_line_up_to_the_budget_and_not_beyond() {
    let one_path_of = |bytes: usize| vec![path(&"a".repeat(bytes - PATHSPEC_OVERHEAD))];
    assert!(fits_the_command_line(&[path("a.txt")]));
    assert!(fits_the_command_line(&one_path_of(PATHSPEC_BUDGET)));
    assert!(!fits_the_command_line(&one_path_of(PATHSPEC_BUDGET + 1)));
    assert!(!fits_the_command_line(&paths_beyond_the_command_line(
        &path("a.txt")
    )));
}

#[test]
fn only_paths_that_fit_become_literal_pathspecs() {
    let args = |pathspecs: Option<&[RepoPath]>| {
        grep_args(
            "q",
            &TreeSource::Worktree,
            5,
            &GrepOptions::default(),
            pathspecs,
        )
    };
    let listed = args(Some(&[path("a*.txt"), path("b.txt")]));
    assert_eq!(
        listed[listed.len() - 3..],
        ["--", ":(literal)a*.txt", ":(literal)b.txt"]
    );
    let unlisted = args(None);
    assert_eq!(unlisted.last().map(String::as_str), Some("--"));
}
