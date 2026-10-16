use std::time::Instant;

use super::*;
use crate::domains::repository::names::object_id::ObjectId;
use crate::domains::repository::process::git_runner::Exit;
use crate::domains::repository::test_repo::TestRepo;

const PROMPT: Duration = Duration::from_secs(5);

fn path(text: &str) -> RepoPath {
    RepoPath::parse(text).unwrap()
}

fn span(start: u64, end: u64) -> LineSpan {
    LineSpan::new(start, end).unwrap()
}

async fn open(fixture: &TestRepo) -> GitRepo {
    GitRepo::open(fixture.path()).await.unwrap()
}

async fn search(
    fixture: &TestRepo,
    query: &str,
    source: &TreeSource,
    options: &GrepOptions,
) -> SearchHits {
    open(fixture)
        .await
        .grep(query, source, 100, options)
        .await
        .unwrap()
}

fn hits(found: &SearchHits) -> Vec<(&str, u64)> {
    found
        .matches
        .iter()
        .map(|found| (found.path.as_str(), found.line))
        .collect()
}

fn at(commit: &ObjectId) -> TreeSource {
    TreeSource::Object(commit.clone())
}

#[tokio::test]
async fn a_fixed_string_is_not_a_pattern_until_regex_is_asked_for() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "foo.bar\nfooxbar\n");
    let source = at(&commit);
    let fixed = search(&fixture, "foo.bar", &source, &GrepOptions::default()).await;
    assert_eq!(hits(&fixed), [("a.txt", 1)]);
    assert_eq!(fixed.matches[0].text, "foo.bar");
    assert!(!fixed.truncated);
    let regex = GrepOptions {
        regex: true,
        ..GrepOptions::default()
    };
    let found = search(&fixture, "foo.bar", &source, &regex).await;
    assert_eq!(hits(&found), [("a.txt", 1), ("a.txt", 2)]);
}

#[tokio::test]
async fn word_and_case_options_narrow_and_widen_the_match() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "cat\nconcat\nCAT\n");
    let source = at(&commit);
    let plain = search(&fixture, "cat", &source, &GrepOptions::default()).await;
    assert_eq!(hits(&plain), [("a.txt", 1), ("a.txt", 2)]);
    let word = GrepOptions {
        word: true,
        ..GrepOptions::default()
    };
    let found = search(&fixture, "cat", &source, &word).await;
    assert_eq!(hits(&found), [("a.txt", 1)]);
    let loose = GrepOptions {
        ignore_case: true,
        word: true,
        ..GrepOptions::default()
    };
    let found = search(&fixture, "cat", &source, &loose).await;
    assert_eq!(hits(&found), [("a.txt", 1), ("a.txt", 3)]);
}

#[tokio::test]
async fn nothing_found_is_an_empty_answer_and_a_bad_regex_is_an_error() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "alpha\n");
    let found = search(&fixture, "zzz", &at(&commit), &GrepOptions::default()).await;
    assert_eq!(found, SearchHits::default());
    let regex = GrepOptions {
        regex: true,
        ..GrepOptions::default()
    };
    let broken = open(&fixture)
        .await
        .grep("(unclosed", &at(&commit), 10, &regex)
        .await;
    assert!(matches!(
        broken,
        Err(GitError::Failed {
            exit: Exit::Failure(_),
            ..
        })
    ));
}

#[tokio::test]
async fn binary_files_are_skipped() {
    let fixture = TestRepo::init();
    fixture.commit("blob.bin", "needle\0needle\n");
    let commit = fixture.commit("text.txt", "needle\n");
    let found = search(&fixture, "needle", &at(&commit), &GrepOptions::default()).await;
    assert_eq!(hits(&found), [("text.txt", 1)]);
}

#[tokio::test]
async fn the_worktree_includes_untracked_files_and_a_commit_ignores_the_checkout() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "committed needle\n");
    std::fs::write(fixture.path().join("a.txt"), "edited\n").unwrap();
    std::fs::write(fixture.path().join("new.txt"), "untracked needle\n").unwrap();
    let options = GrepOptions::default();
    let worktree = search(&fixture, "needle", &TreeSource::Worktree, &options).await;
    assert_eq!(hits(&worktree), [("new.txt", 1)]);
    assert_eq!(worktree.matches[0].text, "untracked needle");
    let committed = search(&fixture, "needle", &at(&commit), &options).await;
    assert_eq!(hits(&committed), [("a.txt", 1)]);
}

#[tokio::test]
async fn paths_restrict_the_search_and_a_colon_in_one_survives_a_commit_prefix() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "needle\n");
    fixture.commit("src/b.txt", "needle\n");
    let commit = fixture.commit("odd:dir/c.txt", "needle\n");
    let only = |names: &[&str]| GrepOptions {
        paths: Some(names.iter().map(|name| path(name)).collect()),
        ..GrepOptions::default()
    };
    let source = at(&commit);
    let found = search(&fixture, "needle", &source, &only(&["src/b.txt"])).await;
    assert_eq!(hits(&found), [("src/b.txt", 1)]);
    let found = search(
        &fixture,
        "needle",
        &source,
        &only(&["odd:dir/c.txt", "a.txt"]),
    )
    .await;
    assert_eq!(hits(&found), [("a.txt", 1), ("odd:dir/c.txt", 1)]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_path_is_a_name_and_never_a_glob() {
    let fixture = TestRepo::init();
    fixture.commit("a*b.txt", "needle\n");
    let commit = fixture.commit("axb.txt", "needle\n");
    let options = GrepOptions {
        paths: Some(vec![path("a*b.txt")]),
        ..GrepOptions::default()
    };
    let found = search(&fixture, "needle", &at(&commit), &options).await;
    assert_eq!(hits(&found), [("a*b.txt", 1)]);
    let worktree = search(&fixture, "needle", &TreeSource::Worktree, &options).await;
    assert_eq!(hits(&worktree), [("a*b.txt", 1)]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_newline_in_a_file_name_does_not_split_its_match() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("odd\nname.txt", "needle\nneedle\n");
    let found = search(&fixture, "needle", &at(&commit), &GrepOptions::default()).await;
    assert_eq!(hits(&found), [("odd\nname.txt", 1), ("odd\nname.txt", 2)]);
}

#[tokio::test]
async fn the_limit_caps_the_matches_and_says_when_there_were_more() {
    let fixture = TestRepo::init();
    fixture.commit("a.txt", "hit 1\nhit 2\nhit 3\nhit 4\nhit 5\n");
    let commit = fixture.commit("b.txt", "hit 6\n");
    let repo = open(&fixture).await;
    let options = GrepOptions::default();
    let two = repo.grep("hit", &at(&commit), 2, &options).await.unwrap();
    assert_eq!(hits(&two), [("a.txt", 1), ("a.txt", 2)]);
    assert!(two.truncated);
    let exact = repo.grep("hit", &at(&commit), 6, &options).await.unwrap();
    assert_eq!(exact.matches.len(), 6);
    assert!(!exact.truncated);
    let zero = repo.grep("hit", &at(&commit), 0, &options).await.unwrap();
    assert!(zero.matches.is_empty());
    assert!(zero.truncated);
}

#[tokio::test]
async fn ranges_filter_before_the_limit_so_early_misses_hide_nothing() {
    let fixture = TestRepo::init();
    let content: String = (1..=12).map(|n| format!("needle {n}\n")).collect();
    let commit = fixture.commit("a.txt", &content);
    fixture.commit("b.txt", &content);
    let options = GrepOptions {
        ranges: Some(HashMap::from([(
            path("a.txt"),
            vec![span(9, 9), span(11, 12)],
        )])),
        ..GrepOptions::default()
    };
    let repo = open(&fixture).await;
    let one = repo
        .grep("needle", &at(&commit), 1, &options)
        .await
        .unwrap();
    assert_eq!(hits(&one), [("a.txt", 9)]);
    assert!(one.truncated);
    let all = repo
        .grep("needle", &at(&commit), 10, &options)
        .await
        .unwrap();
    assert_eq!(hits(&all), [("a.txt", 9), ("a.txt", 11), ("a.txt", 12)]);
    assert!(!all.truncated);
}

#[tokio::test]
async fn nothing_to_search_never_starts_git() {
    let fixture = TestRepo::init();
    let commit = fixture.commit("a.txt", "needle\n");
    let repo = open(&fixture).await;
    // With the repository gone any spawn would fail, so an `Ok` proves Git was not started.
    drop(fixture);
    let source = at(&commit);
    let empty_paths = GrepOptions {
        paths: Some(Vec::new()),
        ..GrepOptions::default()
    };
    let empty_ranges = GrepOptions {
        ranges: Some(HashMap::new()),
        ..GrepOptions::default()
    };
    for (query, options) in [
        ("", GrepOptions::default()),
        ("needle", empty_paths),
        ("needle", empty_ranges),
    ] {
        let found = repo.grep(query, &source, 10, &options).await.unwrap();
        assert_eq!(found, SearchHits::default());
    }
}

#[tokio::test]
async fn a_long_line_is_cut_to_300_characters_without_splitting_one() {
    let fixture = TestRepo::init();
    let line = format!("needle{}{}", "x".repeat(293), "é".repeat(10));
    let commit = fixture.commit("a.txt", &format!("{line}\n"));
    let found = search(&fixture, "needle", &at(&commit), &GrepOptions::default()).await;
    let text = &found.matches[0].text;
    assert_eq!(text.chars().count(), 300);
    assert_eq!(text, &line.chars().take(300).collect::<String>());
    assert!(text.ends_with('é'));
}

#[tokio::test]
async fn stops_git_once_enough_matches_arrived_in_a_huge_file() {
    let fixture = TestRepo::init();
    let content = "needle in a haystack of repeated lines\n".repeat(400_000);
    let commit = fixture.commit("big.txt", &content);
    let everywhere = GrepOptions {
        ranges: Some(HashMap::from([(path("big.txt"), vec![span(1, u64::MAX)])])),
        ..GrepOptions::default()
    };
    let started = Instant::now();
    let found = open(&fixture)
        .await
        .grep("needle", &at(&commit), 1, &everywhere)
        .await
        .unwrap();
    assert_eq!(hits(&found), [("big.txt", 1)]);
    assert!(found.truncated);
    assert!(started.elapsed() < PROMPT);
}

#[path = "grep_long_paths_tests.rs"]
mod long_paths;
