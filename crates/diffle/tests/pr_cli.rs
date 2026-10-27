//! A pull request review run end to end, against a GitHub API on localhost.
mod common;

use std::path::Path;

use common::pr::{Fixture, as_git_url};
use serde_json::json;

/// The whole run: a foreign pull request named by url, served from a clone that goes with SIGTERM.
// POSIX-only: the assertion is on a graceful SIGTERM shutdown, which Windows cannot deliver.
#[cfg(unix)]
// Worker threads keep the stand-in API answering while this test blocks on the child's output.
#[tokio::test(flavor = "multi_thread")]
async fn removes_foreign_clones_when_the_cli_receives_sigterm() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    use axum::Router;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode, Uri};
    use axum::routing::any;

    let f = Fixture::new();
    let url = "https://github.com/foreign/cli/pull/7";
    let node = f.node(json!({ "url": url }));
    // A GitHub API on localhost: the CLI reaches it through GITHUB_API_URL, as in Actions.
    let api = Router::new().fallback(any(move |uri: Uri, headers: HeaderMap, body: Bytes| {
        let node = node.clone();
        async move {
            let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or_default();
            let ok = uri.path() == "/graphql"
                && auth.ends_with(" test-token")
                && String::from_utf8_lossy(&body).contains("pullRequest(");
            if ok {
                (StatusCode::OK, axum::Json(json!({ "data": { "repository": { "pullRequest": node } } })))
            } else {
                (StatusCode::UNAUTHORIZED, axum::Json(json!({ "message": "no" })))
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let config = f.root().join("cli-gitconfig");
    let rewrite = format!("[url \"{}\"]\n\tinsteadOf = https://github.com/foreign/cli\n", as_git_url(&f.origin));
    std::fs::write(&config, rewrite).unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let local = f.local.to_str().unwrap().to_owned();
    let mut child = Command::new(env!("CARGO_BIN_EXE_diffle"))
        .args(["-C", &local, "pr", url, "--no-open", "--no-watch", "--no-lsp", "--port", "0"])
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("NO_COLOR", "1")
        .env("GITHUB_TOKEN", "test-token")
        .env("GITHUB_API_URL", format!("http://{addr}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let mut lines = Vec::new();
    while let Ok(line) = rx.recv_timeout(std::time::Duration::from_secs(60)) {
        let done = line.contains("comparing");
        lines.push(line);
        if done {
            break;
        }
    }
    let root = lines.iter().find_map(|l| l.strip_prefix("📂 repo ")).map(str::to_owned);
    let alive = child.try_wait().unwrap().is_none();
    let _ = Command::new("kill").args(["-TERM", &child.id().to_string()]).status();
    let status = child.wait().unwrap();
    // Whatever the run said while it stopped, for the failure message.
    lines.extend(rx.try_iter());
    let text = lines.join("\n");
    let started = lines.iter().any(|l| l.contains("comparing"));
    assert!(alive && started, "{text}");
    assert!(text.contains("token from GITHUB_TOKEN"), "{text}");
    let root = root.unwrap_or_else(|| panic!("no repo line in {text}"));
    assert_ne!(Path::new(&root), f.local);
    assert_eq!(status.code(), Some(0), "{text}");
    assert!(!Path::new(&root).exists(), "{root} survived the run");
}

/// Resolve fails after the clone exists: the checkout is still removed.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn removes_the_clone_when_resolve_fails_after_a_successful_clone() {
    use std::process::{Command, Stdio};

    use axum::Router;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode, Uri};
    use axum::routing::any;

    let f = Fixture::new();
    // The repository clones. The pull-request head does not, so resolve's fetch fails.
    common::git_in(&f.origin, &["update-ref", "-d", "refs/pull/7/head"]);
    let url = "https://github.com/o/r/pull/7";
    let node = f.node(json!({ "url": url }));
    let api = Router::new().fallback(any(move |uri: Uri, headers: HeaderMap, body: Bytes| {
        let node = node.clone();
        async move {
            let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or_default();
            let ok = uri.path() == "/graphql"
                && auth.ends_with(" test-token")
                && String::from_utf8_lossy(&body).contains("pullRequest(");
            if ok {
                (StatusCode::OK, axum::Json(json!({ "data": { "repository": { "pullRequest": node } } })))
            } else {
                (StatusCode::UNAUTHORIZED, axum::Json(json!({ "message": "no" })))
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let config = f.root().join("cli-gitconfig");
    let rewrite = format!("[url \"{}\"]\n\tinsteadOf = https://github.com/o/r\n", as_git_url(&f.origin));
    std::fs::write(&config, rewrite).unwrap();
    let scratch = f.root().join("scratch");
    std::fs::create_dir(&scratch).unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_diffle"))
        .arg(url)
        .current_dir(outside.path())
        .env("GIT_CONFIG_GLOBAL", &config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("TMPDIR", &scratch)
        .env("XDG_CONFIG_HOME", xdg.path())
        .env("NO_COLOR", "1")
        .env("GITHUB_TOKEN", "test-token")
        .env("GITHUB_API_URL", format!("http://{addr}"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("refs/pull/7/head"), "{err}");
    assert!(!err.contains("comparing"), "{err}");
    let left: Vec<_> = std::fs::read_dir(&scratch).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert!(left.is_empty(), "clone survived resolve failure: {left:?}\n{err}");
}

/// A single positional pull-request URL is a pull request from any directory, on any configured host.
#[test]
fn routes_a_lone_pull_request_url_even_from_outside_a_repository() {
    let outside = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_diffle"))
        .args(["https://github.work.com/acme/app/pull/4", "--no-open", "--no-lsp", "--no-watch"])
        .current_dir(outside.path())
        .env("NO_COLOR", "1")
        .env("XDG_CONFIG_HOME", config.path())
        .env("GIT_CONFIG_GLOBAL", config.path().join("no-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GITHUB_ENTERPRISE_TOKEN", "test-token")
        .env("GH_ENTERPRISE_TOKEN", "")
        .env("GITHUB_API_URL", "http://127.0.0.1:1")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("cannot reach GitHub"), "{err}");
}

/// `https://…/pull/not-a-number` is a pull-request selector that does not parse, not a git revision.
#[test]
fn rejects_a_lone_malformed_pull_request_url() {
    let outside = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let url = "https://github.com/o/r/pull/not-a-number";
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_diffle"))
        .args([url, "--no-open", "--no-lsp", "--no-watch"])
        .current_dir(outside.path())
        .env("NO_COLOR", "1")
        .env("XDG_CONFIG_HOME", config.path())
        .env("GIT_CONFIG_GLOBAL", config.path().join("no-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("pull request url"), "{err}");
    assert!(err.contains(url), "{err}");
    assert!(!err.contains("not a git repository"), "{err}");
}
