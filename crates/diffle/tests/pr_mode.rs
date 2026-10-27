//! Pull requests: naming them, fetching them into this run's refs, and reviewing them in place.
mod common;

use std::future::Future;
use std::sync::Arc;

use common::git_in;
use common::github::{FakeGithub, knows};
use common::pr::{Fixture, as_git_url};
use diffle::cli::repository::open_review_repository_with;
use diffle::comments::CommentStore;
use diffle::error::{Error, Result};
use diffle::git::GitRepo;
use diffle::github::client::GithubClient;
use diffle::github::hosts::{Github, GithubInstance};
use diffle::github::pulls::view_pr;
use diffle::mode::resolve::resolve_review;
use diffle::protocol::{CommentAnchor, ComparisonBase, EnterRequest, Live, RangeView, Review, ThreadQuery};
use diffle::snapshotter::Snapshotter;
use serde_json::json;

fn com() -> GithubInstance {
    GithubInstance::github_com()
}

fn with(client: &FakeGithub) -> Github {
    Github { instance: com(), client: Some(Arc::new(client.clone()) as Arc<dyn GithubClient>) }
}

async fn view(
    selector: Option<&str>,
    repo: Option<&GitRepo>,
    github: &FakeGithub,
    instance: &GithubInstance,
) -> Result<i64> {
    view_pr(selector, repo, github, instance, None).await.map(|pr| pr.number)
}

fn message(e: Error) -> String {
    e.to_string()
}

#[tokio::test]
async fn looks_a_number_a_hash_number_and_a_url_up_by_number_in_the_base_repository() {
    let f = Fixture::new();
    let (repo, github) = (f.repo().await, f.github());
    for selector in ["7", "#7", " 7 ", "https://github.com/o/r/pull/7", "https://github.com/o/r/pull/7/files"] {
        view(Some(selector), Some(&repo), &github, &com()).await.unwrap();
        assert_eq!(github.pop(), json!({ "owner": "o", "repo": "r", "number": 7 }), "{selector}");
    }
    // A url names its own repository: no local checkout needed.
    let other = knows(Some(f.node(json!({ "number": 9 }))));
    view(Some("https://github.com/other/place/pull/9"), None, &other, &com()).await.unwrap();
    assert_eq!(other.pop(), json!({ "owner": "other", "repo": "place", "number": 9 }));
}

#[tokio::test]
async fn looks_a_branch_up_among_the_open_pull_requests_with_that_head_in_any_fork_or_the_named_one() {
    let f = Fixture::new();
    let (repo, github) = (f.repo().await, f.github());
    view(Some("feat"), Some(&repo), &github, &com()).await.unwrap();
    assert_eq!(github.pop(), json!({ "owner": "o", "repo": "r", "head": "feat", "base": null }));
    assert_eq!(view(Some("o:feat"), Some(&repo), &github, &com()).await.unwrap(), 7);
    let err = message(view(Some("someone:feat"), Some(&repo), &github, &com()).await.unwrap_err());
    assert!(err.contains("no open pull request in o/r for someone:feat"), "{err}");
    let err = message(view(Some("feat"), Some(&repo), &knows(None), &com()).await.unwrap_err());
    assert!(err.contains("no open pull request in o/r for feat"), "{err}");
    let (first, second) = (f.node(json!({})), f.node(json!({ "number": 8 })));
    let two = FakeGithub::new(move |_, _| {
        let nodes = json!([first, second]);
        async move { Ok(json!({ "repository": { "pullRequests": { "nodes": nodes } } })) }
    });
    let err = message(view(Some("feat"), Some(&repo), &two, &com()).await.unwrap_err());
    assert!(err.starts_with("2 open pull requests") && err.contains("open one by number"), "{err}");
}

#[tokio::test]
async fn uses_the_checked_out_branchs_upstream_when_nothing_is_given() {
    let f = Fixture::new();
    let work = f.root().join("branch");
    git_in(&f.root(), &["clone", "-q", &as_git_url(&f.origin), &as_git_url(&work)]);
    f.point_remote_at_github(&work, "origin");
    git_in(&work, &["checkout", "-q", "-b", "feat"]);
    git_in(&work, &["branch", "-q", "--set-upstream-to=origin/main"]);
    let (repo, github) = (GitRepo::open(&work).await.unwrap(), f.github());
    // The upstream names the branch on GitHub; the local name may differ.
    assert_eq!(view(None, Some(&repo), &github, &com()).await.unwrap(), 7);
    assert_eq!(github.pop(), json!({ "owner": "o", "repo": "r", "head": "main", "base": null }));
    git_in(&work, &["branch", "-q", "--unset-upstream"]);
    // Without an upstream, the local branch name is the best guess.
    view(None, Some(&repo), &github, &com()).await.unwrap();
    assert_eq!(github.pop(), json!({ "owner": "o", "repo": "r", "head": "feat", "base": null }));
    git_in(&work, &["checkout", "-q", "--detach"]);
    assert!(message(view(None, Some(&repo), &github, &com()).await.unwrap_err()).contains("not on a branch"));
}

#[tokio::test]
async fn refuses_an_option_shaped_selector_and_a_non_pull_request_url_before_asking_github() {
    let f = Fixture::new();
    let (repo, github) = (f.repo().await, f.github());
    assert!(matches!(view(Some("--json"), Some(&repo), &github, &com()).await, Err(Error::Github(_))));
    let err = message(view(Some("https://github.com/o/r/issues/7"), Some(&repo), &github, &com()).await.unwrap_err());
    assert!(err.contains("not a pull request url"), "{err}");
    assert!(github.calls().is_empty());
}

#[tokio::test]
async fn needs_a_github_origin_or_one_github_remote_for_everything_but_a_url() {
    let f = Fixture::new();
    let github = f.github();
    assert!(message(view(Some("7"), None, &github, &com()).await.unwrap_err()).contains("not in a git repository"));
    let dir = f.root().join("no-origin");
    git_in(&f.root(), &["init", "-q", &as_git_url(&dir)]);
    let open = || async { GitRepo::open(&dir).await.unwrap() };
    assert!(
        message(view(Some("7"), Some(&open().await), &github, &com()).await.unwrap_err())
            .contains("no remote on github.com")
    );
    git_in(&dir, &["remote", "add", "fork", "git@github.com:me/r.git"]);
    assert_eq!(view(Some("7"), Some(&open().await), &github, &com()).await.unwrap(), 7);
    assert_eq!(github.pop(), json!({ "owner": "me", "repo": "r", "number": 7 }));
    git_in(&dir, &["remote", "add", "other", "https://github.com/o/r"]);
    let err = message(view(Some("7"), Some(&open().await), &github, &com()).await.unwrap_err());
    assert!(err.contains("2 GitHub remotes and no origin"), "{err}");
    assert!(github.calls().is_empty());
}

#[tokio::test]
async fn reads_remotes_and_urls_on_the_runs_github_enterprise_host_alone() {
    let f = Fixture::new();
    let github = f.github();
    let enterprise = GithubInstance {
        host: "github.work.com".into(),
        git_hosts: vec!["github.work.com".into(), "git.work.com".into()],
    };
    let dir = f.root().join("enterprise");
    git_in(&f.root(), &["init", "-q", &as_git_url(&dir)]);
    git_in(&dir, &["remote", "add", "origin", "git@git.work.com:team/svc.git"]);
    let work = GitRepo::open(&dir).await.unwrap();
    view(Some("7"), Some(&work), &github, &enterprise).await.unwrap();
    assert_eq!(github.pop(), json!({ "owner": "team", "repo": "svc", "number": 7 }));
    assert!(
        message(view(Some("7"), Some(&work), &github, &com()).await.unwrap_err()).contains("no remote on github.com")
    );
    let err =
        message(view(Some("https://github.com/o/r/pull/7"), Some(&work), &github, &enterprise).await.unwrap_err());
    assert!(err.contains("is on github.com, but diffle is talking to github.work.com"), "{err}");
    assert!(github.calls().is_empty());
}

#[tokio::test]
async fn normalizes_the_head_repository_and_accepts_a_deleted_fork() {
    let f = Fixture::new();
    let repo = f.repo().await;
    let pr = view_pr(Some("7"), Some(&repo), &f.github(), &com(), None).await.unwrap();
    assert_eq!(pr.head_repository.as_deref(), Some("o/r"));
    let deleted = knows(Some(f.node(json!({ "headRepository": null }))));
    assert_eq!(view_pr(Some("7"), Some(&repo), &deleted, &com(), None).await.unwrap().head_repository, None);
}

#[tokio::test]
async fn rejects_malformed_pr_fields() {
    let f = Fixture::new();
    let repo = f.repo().await;
    let mut cases = vec![
        f.node(json!({ "headRepository": { "name": "r", "owner": { "login": 3 } } })),
        f.node(json!({ "headRepository": { "name": "r" } })),
        f.node(json!({ "state": "UNKNOWN" })),
        f.node(json!({ "headRefOid": null })),
    ];
    let mut missing = f.node(json!({}));
    missing.as_object_mut().unwrap().remove("headRepository");
    cases.push(missing);
    for node in cases {
        let err = view_pr(Some("7"), Some(&repo), &knows(Some(node.clone())), &com(), None).await.unwrap_err();
        let Error::Github(e) = err else { panic!("{node}") };
        assert_eq!((e.status, e.message.as_str()), (502, "unexpected pull request data from GitHub"), "{node}");
    }
}

#[tokio::test]
async fn names_unusable_github_data_instead_of_guessing() {
    let f = Fixture::new();
    let repo = f.repo().await;
    assert!(
        message(view(Some("7"), Some(&repo), &knows(None), &com()).await.unwrap_err())
            .contains("no pull request o/r#7")
    );
    let elsewhere = knows(Some(f.node(json!({ "url": "https://example.invalid/x" }))));
    let err = message(view(Some("7"), Some(&repo), &elsewhere, &com()).await.unwrap_err());
    assert!(err.contains("cannot read the repository"), "{err}");
}

fn pr(selector: &str) -> EnterRequest {
    EnterRequest::Pr { pr: Some(selector.into()) }
}

#[tokio::test]
async fn fetches_the_pull_request_into_fixed_refs_retaining_both_branch_names() {
    let f = Fixture::new();
    let repo = f.repo().await;
    let repo = Arc::new(repo);
    let review = resolve_review(&pr("7"), &repo, &with(&f.github())).await.unwrap();
    let Review::Range { old, new, base, pr: pull, view } = &review else { panic!("a range") };
    assert_eq!(*old, format!("{}/7/base/main", repo.review_refs));
    assert_eq!(*new, format!("{}/7/head/feat", repo.review_refs));
    assert_eq!(*base, ComparisonBase::MergeBase);
    assert!(matches!(view, RangeView::Whole));
    let pull = pull.as_ref().unwrap();
    assert_eq!((pull.url.as_str(), pull.number, pull.repository.as_str()), ("https://github.com/o/r/pull/7", 7, "o/r"));
    // Branch identities share review state with ordinary branch comparisons; the fetched refs stay put.
    let key = common::range_key(&repo, &review).await;
    assert_eq!(key.as_deref(), Some("range:o/r:main...o/r:feat"));
    let snap = Snapshotter::new(repo.clone(), review.clone(), key.clone(), 1, 3, None).current().await.unwrap();
    assert_eq!((snap.follows, Some(snap.comment_key.clone())), (Live::None, key));
    assert_eq!(git_in(&f.local, &["rev-parse", new]), f.head);
    assert_eq!(git_in(&f.local, &["rev-parse", old]), f.merge_base);
    repo.clean_review_refs().await.unwrap();
}

#[tokio::test]
async fn fetches_from_the_remote_that_points_at_the_base_repository_whatever_it_is_called() {
    let f = Fixture::new();
    let forked = f.root().join("forked");
    git_in(&f.root(), &["clone", "-q", "--origin", "upstream", &as_git_url(&f.origin), &as_git_url(&forked)]);
    f.point_remote_at_github(&forked, "upstream");
    let repo = GitRepo::open(&forked).await.unwrap();
    let review = resolve_review(&pr("7"), &repo, &with(&f.github())).await.unwrap();
    let Review::Range { new, .. } = &review else { panic!("a range") };
    assert_eq!(repo.resolve(new).await.unwrap(), f.head);
}

#[tokio::test]
async fn refuses_foreign_prs_in_an_existing_session_without_fetching() {
    let f = Fixture::new();
    let repo = f.repo().await;
    let refs = git_in(&f.local, &["show-ref"]);
    let foreign = knows(Some(f.node(json!({ "url": "https://github.com/foreign/repo/pull/7" }))));
    let err = resolve_review(&pr("7"), &repo, &with(&foreign)).await.unwrap_err();
    assert!(err.to_string().contains("foreign repository"), "{err}");
    assert_eq!(git_in(&f.local, &["show-ref"]), refs);
}

#[tokio::test]
async fn cleans_only_this_instances_refs() {
    let f = Fixture::new();
    let (repo, other) = (f.repo().await, f.repo().await);
    resolve_review(&pr("7"), &other, &with(&f.github())).await.unwrap();
    repo.clean_review_refs().await.unwrap();
    assert_eq!(git_in(&f.local, &["for-each-ref", &repo.review_refs]), "");
    assert_eq!(git_in(&f.local, &["rev-parse", &format!("{}/7/head/feat", other.review_refs)]), f.head);
    other.clean_review_refs().await.unwrap();
}

#[tokio::test]
async fn is_off_without_a_token() {
    let f = Fixture::new();
    let repo = f.repo().await;
    let err = resolve_review(&pr("7"), &repo, &Github::tokenless()).await.unwrap_err();
    assert!(err.to_string().contains("No GitHub token"), "{err}");
    let mut req = pr("7");
    let opened = open_review_repository_with(&mut req, &f.local, async |_| Ok((Github::tokenless(), None))).await;
    assert!(opened.err().unwrap().to_string().contains("No GitHub token"));
}

fn git_env() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn block_on<F: Future>(future: F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(future)
}

/// Sets a process variable and puts the previous value back, including when it was unset.
struct EnvVar {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVar {
    fn set(name: &'static str, value: &std::path::Path) -> Self {
        let previous = std::env::var_os(name);
        // SAFETY: the matching tests hold `git_env` for this whole body, and `git_in` overrides it in children.
        unsafe { std::env::set_var(name, value) }
        Self { name, previous }
    }
}

impl Drop for EnvVar {
    fn drop(&mut self) {
        // SAFETY: as in `set`.
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.name, value),
                None => std::env::remove_var(self.name),
            }
        }
    }
}

#[test]
fn isolates_a_matching_pull_request_from_the_callers_checkout() {
    let f = Fixture::new();
    std::fs::write(f.local.join("a.txt"), "one\nlocal\n").unwrap();
    std::fs::write(f.local.join("staged.txt"), "staged\n").unwrap();
    git_in(&f.local, &["add", "staged.txt"]);
    std::fs::write(f.local.join("extra.txt"), "untracked\n").unwrap();
    let head = git_in(&f.local, &["rev-parse", "HEAD"]);
    let index = git_in(&f.local, &["diff", "--cached"]);
    let worktree = git_in(&f.local, &["diff"]);
    let refs = git_in(&f.local, &["show-ref"]);
    let extra = std::fs::read(f.local.join("extra.txt")).unwrap();
    let config = f.root().join("isolate-gitconfig");
    let rewrite = format!("[url \"{}\"]\n\tinsteadOf = https://github.com/o/r\n", as_git_url(&f.origin));
    std::fs::write(&config, &rewrite).unwrap();
    // The guard stays in this synchronous test. The awaits run in `block_on`, so it is not held inside the future.
    let _lock = git_env();
    let _env = EnvVar::set("GIT_CONFIG_GLOBAL", &config);

    let (req, clone_root, local_exists) = block_on(async {
        let github = with(&f.github());
        let mut req = pr("7");
        let opened = open_review_repository_with(&mut req, &f.local, async |_| Ok((github, None))).await.unwrap();
        assert_ne!(opened.repo.root, f.local, "a matching remote is still cloned");
        assert!(opened.clone.is_some());
        let clone_root = opened.repo.root.clone();
        drop(opened);
        (req, clone_root, f.local.exists())
    });
    let EnterRequest::Pr { pr: Some(url) } = &req else { panic!("a pull request") };
    assert_eq!(url, "https://github.com/o/r/pull/7");
    assert_eq!(git_in(&f.local, &["rev-parse", "HEAD"]), head);
    assert_eq!(git_in(&f.local, &["diff", "--cached"]), index);
    assert_eq!(git_in(&f.local, &["diff"]), worktree);
    assert_eq!(git_in(&f.local, &["show-ref"]), refs);
    assert_eq!(std::fs::read(f.local.join("extra.txt")).unwrap(), extra);
    assert!(!clone_root.exists(), "{}", clone_root.display());
    assert!(local_exists);
}

#[test]
fn reopens_pull_request_comments_outside_the_clone() {
    let f = Fixture::new();
    let config = f.root().join("durable-gitconfig");
    let rewrite = format!("[url \"{}\"]\n\tinsteadOf = https://github.com/o/r\n", as_git_url(&f.origin));
    std::fs::write(&config, rewrite).unwrap();
    let xdg = f.root().join("xdg");
    std::fs::create_dir(&xdg).unwrap();
    // The guard stays in this synchronous test. The awaits run in `block_on`, so it is not held inside the future.
    let _lock = git_env();
    let _git = EnvVar::set("GIT_CONFIG_GLOBAL", &config);
    let _xdg = EnvVar::set("XDG_CONFIG_HOME", &xdg);
    block_on(async {
        // A host of `..` must stay inside the reviews directory.
        let github = || Github {
            instance: GithubInstance { host: "..".into(), git_hosts: vec!["..".into(), "github.com".into()] },
            client: Some(Arc::new(f.github()) as Arc<dyn GithubClient>),
        };
        let mut req = pr("https://github.com/o/r/pull/7");
        let opened = open_review_repository_with(&mut req, &f.local, async |_| Ok((github(), None))).await.unwrap();
        let state = opened.repo.review_state_dir().to_path_buf();
        let reviews = xdg.join("diffle").join("reviews");
        std::fs::create_dir_all(&reviews).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let canon = dunce::canonicalize(&state).unwrap();
        let reviews = dunce::canonicalize(&reviews).unwrap();
        assert!(canon.starts_with(&reviews), "{}", canon.display());
        assert!(!canon.starts_with(dunce::canonicalize(&opened.repo.root).unwrap()));
        let store = CommentStore::open(&state, "pull").await.unwrap();
        store.add_thread(CommentAnchor::File { path: "a.txt".into() }, "kept".into()).await.unwrap();
        store.set_viewed("a.txt", "abc", true).await.unwrap();
        drop(store);
        drop(opened);
        let mut req = pr("7");
        let opened = open_review_repository_with(&mut req, &f.local, async |_| Ok((github(), None))).await.unwrap();
        assert_eq!(opened.repo.review_state_dir(), state.as_path());
        let store = CommentStore::open(opened.repo.review_state_dir(), "pull").await.unwrap();
        let threads = store.threads(&ThreadQuery { path: None, state: None }).await.unwrap();
        assert_eq!(threads[0].messages[0].body, "kept");
        assert!(store.viewed().await.unwrap().iter().any(|mark| mark.path == "a.txt" && mark.viewed));
    });
}

#[tokio::test]
async fn refuses_a_refspec_that_would_move_a_ref_the_user_owns() {
    let f = Fixture::new();
    let repo = f.repo().await;
    for spec in ["+refs/heads/main:refs/heads/main", "+refs/heads/main:refs/remotes/origin/main"] {
        let err = repo.fetch("origin", &[spec.to_owned()]).await.unwrap_err();
        assert!(err.to_string().starts_with("refusing to fetch into"), "{err}");
    }
}
