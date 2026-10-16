//! A trusted public origin behind a reverse proxy: scheme, host and port, nothing else.

use std::fmt;

use thiserror::Error;

const HTTP: &str = "http";
const HTTPS: &str = "https";
const SCHEME_SEPARATOR: &str = "://";
const DEFAULT_HTTP_PORT: u16 = 80;
const DEFAULT_HTTPS_PORT: u16 = 443;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("expected an HTTP(S) origin, e.g. https://proxy.example (without a path)")]
pub struct InvalidOrigin;

/// The origin as a browser sends it in `Origin`: lowercase, default port left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedOrigin {
    origin: String,
    host: String,
}

impl AllowedOrigin {
    /// Accepts an HTTP(S) origin, with at most a trailing `/`: no credentials, path, query
    /// or fragment, which a proxy cannot make part of an origin.
    pub fn parse(text: &str) -> Result<Self, InvalidOrigin> {
        let bare = text.strip_suffix('/').unwrap_or(text);
        let (scheme, authority) = split_origin(bare).ok_or(InvalidOrigin)?;
        if authority.contains(['/', '?', '#', '@']) {
            return Err(InvalidOrigin);
        }
        let host = normalized_host(scheme, authority).ok_or(InvalidOrigin)?;
        Ok(Self {
            origin: format!("{scheme}{SCHEME_SEPARATOR}{host}"),
            host,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.origin
    }

    /// The `Host` a proxy forwards for this origin: host and port.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }
}

impl fmt::Display for AllowedOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.origin)
    }
}

/// The host and port a request's `Origin` header names, normalized as a browser would write
/// them; `None` for `null` and anything that is not a URL with an authority.
#[must_use]
pub fn origin_host(origin: &str) -> Option<String> {
    let (scheme, rest) = split_origin(origin)?;
    let authority = rest.split(['/', '?', '#']).next()?;
    normalized_host(scheme, authority.rsplit('@').next()?)
}

fn split_origin(text: &str) -> Option<(&'static str, &str)> {
    let lower_scheme = |scheme: &'static str| {
        let prefix = text.get(..scheme.len())?;
        let rest = text.get(scheme.len()..)?.strip_prefix(SCHEME_SEPARATOR)?;
        prefix
            .eq_ignore_ascii_case(scheme)
            .then_some((scheme, rest))
    };
    lower_scheme(HTTPS).or_else(|| lower_scheme(HTTP))
}

/// `host[:port]` lowercase, with the scheme's default port dropped; `None` when empty or the
/// port is not a number.
fn normalized_host(scheme: &str, authority: &str) -> Option<String> {
    let (host, port) = match authority.strip_prefix('[') {
        Some(rest) => {
            let (inside, after) = rest.split_once(']')?;
            (format!("[{inside}]"), after.strip_prefix(':'))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host.to_owned(), Some(port)),
            None => (authority.to_owned(), None),
        },
    };
    if host.is_empty() || host == "[]" {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let default = match scheme {
        HTTPS => DEFAULT_HTTPS_PORT,
        _ => DEFAULT_HTTP_PORT,
    };
    match port.map(str::parse::<u16>).transpose().ok()? {
        Some(port) if port != default => Some(format!("{host}:{port}")),
        Some(_) | None => Some(host),
    }
}

#[cfg(test)]
#[path = "allowed_origin_tests.rs"]
mod tests;
