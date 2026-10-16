use clap::Parser;

use super::*;
use crate::domains::settings::follow_refs::FollowRefs;

fn invocation(args: &[&str]) -> Invocation {
    let cli = Cli::try_parse_from(std::iter::once("diffle").chain(args.iter().copied())).unwrap();
    Invocation::from_cli(cli).unwrap()
}

fn config(args: &[&str]) -> Config {
    match invocation(args) {
        Invocation::Review(config) => *config,
        Invocation::Settings(action) => panic!("expected a review, got {action:?}"),
    }
}

fn shown(config: &Config) -> String {
    match config.request() {
        EntryRequest::Working => "working".to_owned(),
        EntryRequest::Revspec(spec) => spec.to_string(),
    }
}

fn usage_error(args: &[&str]) -> String {
    Cli::try_parse_from(std::iter::once("diffle").chain(args.iter().copied()))
        .unwrap_err()
        .to_string()
}

#[test]
fn no_arguments_review_the_working_tree_here() {
    let config = config(&[]);
    assert_eq!(shown(&config), "working");
    assert_eq!(config.directory(), Path::new("."));
}

#[test]
fn revisions_become_a_revspec() {
    assert_eq!(shown(&config(&["main"])), "main...HEAD");
    assert_eq!(shown(&config(&["a", "b"])), "a..b");
}

#[test]
fn the_shorthands_name_the_common_comparisons() {
    assert_eq!(shown(&config(&["working"])), "working");
    assert_eq!(shown(&config(&["show"])), "HEAD^!");
    assert_eq!(shown(&config(&["show", "a1b2c3d"])), "a1b2c3d^!");
}

#[test]
fn the_directory_flag_works_beside_a_shorthand() {
    let config = config(&["-C", "/repo", "show", "main"]);
    assert_eq!(config.directory(), Path::new("/repo"));
    assert_eq!(shown(&config), "main^!");
}

#[test]
fn bad_revisions_are_the_reviewers_to_fix() {
    let cli = Cli::try_parse_from(["diffle", "a..b", "c"]).unwrap();
    assert_eq!(
        Invocation::from_cli(cli),
        Err(RevspecError::RangeWithTwoRevisions)
    );
}

#[test]
fn the_defaults_serve_loopback_on_the_first_free_port_and_end_with_the_tab() {
    let config = config(&[]);
    assert_eq!(config.host(), "127.0.0.1");
    assert_eq!(config.port(), PortChoice::Probe);
    assert_eq!(config.browser(), Browser::EndsWithTab);
    assert!(config.watch());
    assert!(config.auto_viewed().is_empty());
    assert_eq!(config.context(), None);
    assert_eq!(config.allowed_origin(), None);
}

#[test]
fn the_serving_flags_are_read_before_or_after_the_revisions() {
    let config = config(&[
        "-p",
        "0",
        "main",
        "--host",
        "0.0.0.0",
        "--no-watch",
        "--auto-viewed",
        "*.lock",
        "--auto-viewed",
        "dist/**",
        "-U",
        "12",
        "--allowed-origin",
        "https://review.example.com/",
        "--client-dir",
        "/srv/client",
    ]);
    assert_eq!(shown(&config), "main...HEAD");
    assert_eq!(config.port(), PortChoice::Exact(0));
    assert_eq!(config.host(), "0.0.0.0");
    assert!(!config.watch());
    assert_eq!(config.auto_viewed(), ["*.lock", "dist/**"]);
    assert_eq!(config.context().map(ContextLines::get), Some(12));
    assert_eq!(
        config.allowed_origin().map(AllowedOrigin::as_str),
        Some("https://review.example.com")
    );
    assert_eq!(config.client_dir(), Some(Path::new("/srv/client")));
}

#[test]
fn a_flag_after_a_shorthand_still_applies() {
    let config = config(&["show", "main", "--no-open"]);
    assert_eq!(config.browser(), Browser::NotOpened);
    assert_eq!(shown(&config), "main^!");
}

#[test]
fn the_browser_policy_follows_the_two_flags() {
    assert_eq!(config(&["--no-open"]).browser(), Browser::NotOpened);
    assert_eq!(config(&["--keep-alive"]).browser(), Browser::Kept);
    assert_eq!(
        config(&["--no-open", "--keep-alive"]).browser(),
        Browser::NotOpened
    );
    assert!(!Browser::NotOpened.opens());
    assert!(Browser::Kept.opens() && !Browser::Kept.ends_with_tab());
    assert!(Browser::EndsWithTab.opens() && Browser::EndsWithTab.ends_with_tab());
}

#[test]
fn bad_flag_values_are_usage_errors_that_say_what_was_expected() {
    assert!(usage_error(&["-U", "2.5"]).contains("contextLines must be a whole number"));
    assert!(usage_error(&["-U", "many"]).contains("expected a whole number"));
    assert!(usage_error(&["--allowed-origin", "ftp://x"]).contains("expected an HTTP(S) origin"));
    assert!(usage_error(&["--port", "70000"]).contains("70000"));
}

#[test]
fn config_subcommands_edit_the_preferences_instead_of_serving() {
    assert_eq!(
        invocation(&["config"]),
        Invocation::Settings(ConfigAction::Show)
    );
    assert_eq!(
        invocation(&["config", "add-auto-viewed", "*.lock", "dist/**"]),
        Invocation::Settings(ConfigAction::AddAutoViewed {
            globs: vec!["*.lock".to_owned(), "dist/**".to_owned()]
        })
    );
    assert_eq!(
        invocation(&["config", "set-follow-refs", "off"]),
        Invocation::Settings(ConfigAction::SetFollowRefs {
            mode: FollowRefs::Off
        })
    );
    assert!(usage_error(&["config", "set-follow-refs", "sometimes"]).contains("on, auto or off"));
}
