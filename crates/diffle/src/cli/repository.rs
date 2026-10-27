//! The repository a run reviews, and the GitHub it talks to: local, or an isolated pull-request clone.
use std::path::Path;
use std::sync::Arc;

use crate::error::{Error, GitError, GithubError, Result};
use crate::git::GitRepo;
use crate::github::client::{HttpClient, no_token, resolve_token};
use crate::github::hosts::{Github, choose_instance, github_repository};
use crate::github::pulls::view_pr;
use crate::persist::user_config_dir;
use crate::protocol::EnterRequest;

/// Child of the user config directory that holds pull-request review state, outside any clone.
const PR_REVIEW_STATE: &str = "reviews";

pub struct Opened {
    pub repo: Arc<GitRepo>,
    pub github: Github,
    /// Where the token came from, for the startup line.
    pub token_source: Option<String>,
    /// The pull request's disposable clone. The run closes it after git has stopped.
    pub clone: Option<tempfile::TempDir>,
}

fn is_url(selector: &str) -> bool {
    let lower = selector.to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Opens what `req` reviews from `cwd`. Every pull request is cloned into a disposable directory and
/// never fetched into the caller's repository; `req` then names the pull request by its url.
pub async fn open_review_repository(
    req: &mut EnterRequest,
    cwd: &Path,
    github_hosts: &std::collections::HashMap<String, String>,
) -> Result<Opened> {
    let selector = match &*req {
        EnterRequest::Pr { pr } => pr.clone(),
        _ => None,
    };
    open_review_repository_with(req, cwd, async |local| connect_github(github_hosts, local, selector.as_deref()).await)
        .await
}

/// `open_review_repository`, finding the run's GitHub through `connect` from the local repository,
/// when there is one. Test seam.
pub async fn open_review_repository_with(
    req: &mut EnterRequest,
    cwd: &Path,
    connect: impl AsyncFnOnce(Option<&GitRepo>) -> Result<(Github, Option<String>)>,
) -> Result<Opened> {
    let selector = match &*req {
        EnterRequest::Pr { pr } => pr.clone(),
        _ => None,
    };
    let local = match GitRepo::open(cwd).await {
        Ok(repo) => Some(repo),
        // Outside a repository only a pull request url can say what to review.
        Err(Error::Git(e)) if e.code == Some(128) && selector.as_deref().is_some_and(is_url) => None,
        Err(e) => return Err(e),
    };
    let (github, token_source) = connect(local.as_ref()).await?;
    let opened = |repo: GitRepo, github: Github, clone| Opened {
        repo: Arc::new(repo),
        github,
        token_source: token_source.clone(),
        clone,
    };
    let EnterRequest::Pr { pr } = req else {
        let local = local
            .ok_or_else(|| GitError::refused(format!("not a git repository: {}", cwd.display()), &["rev-parse"]))?;
        return Ok(opened(local, github, None));
    };
    let Some(client) = github.client.clone() else {
        return Err(GithubError::conflict(no_token(&github.instance.host)).into());
    };
    let found = view_pr(pr.as_deref(), local.as_ref(), client.as_ref(), &github.instance, None).await?;
    let mut source = clone_url(&found.url)?;
    if let Some(local) = &local {
        let remotes = local.remotes().await?;
        if let Some(remote) = remotes.into_iter().find(|remote| {
            github_repository(&remote.url, &github.instance)
                .is_some_and(|name| name.eq_ignore_ascii_case(&found.repository))
        }) {
            source = remote.url;
        }
    }
    let dir = tempfile::Builder::new().prefix("diffle-pr-").tempdir()?;
    let mut repo = GitRepo::clone_into(&source, dir.path()).await?;
    // Before the repository is shared: comments for this host and repository outlive the clone.
    repo.set_review_state_dir(durable_review_dir(&github.instance.host, &found.repository)?);
    *pr = Some(found.url);
    Ok(opened(repo, github, Some(dir)))
}

/// The base repository URL of a pull request, without its `/pull/<n>` suffix.
fn clone_url(pr_url: &str) -> Result<String> {
    let at = pr_url
        .to_ascii_lowercase()
        .rfind("/pull/")
        .filter(|at| *at > 0)
        .ok_or_else(|| GithubError::new(format!("not a pull request url: {pr_url}"), 400))?;
    Ok(pr_url[..at].to_owned())
}

/// `<config>/reviews/<host>/<owner>/<name>`. Each segment is encoded so it cannot be `.`, `..`, or
/// a path separator.
fn durable_review_dir(host: &str, repository: &str) -> Result<std::path::PathBuf> {
    let (owner, name) = repository
        .split_once('/')
        .filter(|(owner, name)| !owner.is_empty() && !name.is_empty() && !name.contains('/'))
        .ok_or_else(|| GithubError::new(format!("cannot store review state for {repository}"), 502))?;
    Ok(user_config_dir()
        .join(PR_REVIEW_STATE)
        .join(path_segment(host))
        .join(path_segment(owner))
        .join(path_segment(name)))
}

fn path_segment(raw: &str) -> String {
    let mut out = String::new();
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    if matches!(out.as_str(), "" | "." | "..") {
        out.insert(0, '_');
    }
    out
}

/// One GitHub and one token lookup per run; without a token, GitHub features are off rather than
/// failing later. `GITHUB_API_URL`, as Actions sets it, names the run's API outright.
async fn connect_github(
    github_hosts: &std::collections::HashMap<String, String>,
    local: Option<&GitRepo>,
    selector: Option<&str>,
) -> Result<(Github, Option<String>)> {
    let instance = choose_instance(github_hosts, local, selector).await;
    let env = |name: &str| std::env::var(name).ok();
    let Some((token, source)) = resolve_token(&instance.host, env).await else {
        return Ok((Github { instance, client: None }, None));
    };
    let base = env("GITHUB_API_URL").filter(|u| !u.is_empty()).unwrap_or_else(|| instance.api_base_url());
    let client = HttpClient::new(token, &base)?;
    Ok((Github { instance, client: Some(Arc::new(client)) }, Some(source)))
}
