//! The hosted repository a remote URL points at, so a review is keyed by the repository
//! rather than by the clone's local remote names.

const HOST: &str = "github.com";

/// `owner/name` on GitHub, as written in a remote URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedRepo {
    owner: String,
    name: String,
}

impl HostedRepo {
    /// The repository behind an HTTPS, `ssh://`, or scp-style GitHub remote URL; `None` for
    /// any other host or shape.
    pub fn parse_github(url: &str) -> Option<Self> {
        let path = https_path(url)
            .or_else(|| ssh_path(url))
            .or_else(|| scp_path(url))?;
        let path = path.strip_suffix('/').unwrap_or(path);
        let (owner, name) = path.split_once('/')?;
        let name = strip_git_suffix(name);
        if owner.is_empty() || name.is_empty() || name.contains('/') {
            return None;
        }
        Some(Self {
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    /// `owner/name` in lowercase: GitHub names compare case-insensitively.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name).to_lowercase()
    }
}

fn https_path(url: &str) -> Option<&str> {
    strip_prefix_ci(url, &format!("https://{HOST}/"))
        .or_else(|| strip_prefix_ci(url, &format!("http://{HOST}/")))
}

fn ssh_path(url: &str) -> Option<&str> {
    let rest = without_user(strip_prefix_ci(url, "ssh://")?);
    let rest = strip_prefix_ci(rest, HOST)?;
    // An optional `:port` between the host and the path.
    let rest = match rest.split_once('/') {
        Some((port, path)) => match port.strip_prefix(':') {
            Some(digits) if digits.bytes().all(|b| b.is_ascii_digit()) => path,
            Some(_) => return None,
            None if port.is_empty() => path,
            None => return None,
        },
        None => return None,
    };
    Some(rest)
}

fn scp_path(url: &str) -> Option<&str> {
    strip_prefix_ci(without_user(url), &format!("{HOST}:"))
}

/// Drops a leading `user@` when the user is a plain name.
fn without_user(text: &str) -> &str {
    match text.split_once('@') {
        Some((user, rest)) if !user.is_empty() && !user.contains('/') => rest,
        Some(_) | None => text,
    }
}

fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| text.get(prefix.len()..))?
}

fn strip_git_suffix(name: &str) -> &str {
    const SUFFIX: &str = ".git";
    match name
        .len()
        .checked_sub(SUFFIX.len())
        .and_then(|at| name.get(at..))
    {
        Some(tail) if tail.eq_ignore_ascii_case(SUFFIX) => {
            name.get(..name.len() - SUFFIX.len()).unwrap_or(name)
        }
        Some(_) | None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(url: &str) -> Option<String> {
        HostedRepo::parse_github(url).map(|repo| repo.slug())
    }

    #[test]
    fn reads_every_github_url_shape() {
        for url in [
            "https://github.com/Owner/Repo",
            "https://github.com/Owner/Repo.git",
            "http://GitHub.com/Owner/Repo/",
            "git@github.com:Owner/Repo.git",
            "github.com:Owner/Repo",
            "ssh://git@github.com/Owner/Repo",
            "ssh://git@github.com:22/Owner/Repo.GIT",
        ] {
            assert_eq!(slug(url).as_deref(), Some("owner/repo"), "{url}");
        }
    }

    #[test]
    fn rejects_other_hosts_and_shapes() {
        for url in [
            "https://gitlab.com/o/r",
            "https://github.com/o",
            "https://github.com/o/r/extra",
            "ssh://git@github.com:abc/o/r",
            "/home/me/repo",
            "",
        ] {
            assert_eq!(slug(url), None, "{url}");
        }
    }
}
