//! Which `Host` and `Origin` may talk to the API: a page on another site must not drive a
//! server that sits on loopback (DNS rebinding, cross-site requests).

use std::collections::HashSet;
use std::net::IpAddr;

use thiserror::Error;

use super::allowed_origin::{AllowedOrigin, origin_host};

const LOOPBACK: [&str; 3] = ["127.0.0.1", "::1", "localhost"];
const UNCONFIGURED_HINT: &str =
    "if a reverse proxy sent it, trust that origin with --allowed-origin";
const MDNS_SUFFIX: &str = ".local";

/// Why a request was turned away; the message names the header and how to get it trusted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Rejection {
    #[error("missing Host header")]
    MissingHost,
    #[error("forbidden Host \"{host}\"; {hint}")]
    ForbiddenHost { host: String, hint: String },
    #[error("forbidden Origin \"{origin}\" for Host \"{host}\"; {hint}")]
    ForbiddenOrigin {
        origin: String,
        host: String,
        hint: String,
    },
}

/// Restricts API and WebSocket requests to local hosts and the configured public origin.
/// Forwarded headers never grant trust: a public origin must be configured explicitly.
#[derive(Debug, Clone)]
pub struct RequestGuard {
    loopback_only: bool,
    names: HashSet<String>,
    public: Option<AllowedOrigin>,
}

impl RequestGuard {
    /// `own_names` are the machine's names a LAN visitor may use; they only count when the
    /// server is bound beyond loopback.
    #[must_use]
    pub fn new(bind_host: &str, own_names: &[String], public: Option<AllowedOrigin>) -> Self {
        let bind = bind_host.to_ascii_lowercase();
        let names = LOOPBACK
            .iter()
            .map(|name| (*name).to_owned())
            .chain(own_names.iter().map(|name| name.to_ascii_lowercase()))
            .chain(std::iter::once(bind.clone()))
            .collect();
        Self {
            loopback_only: LOOPBACK.contains(&bind.as_str()),
            names,
            public,
        }
    }

    /// Accepts a request whose `Host` is local (or the configured public host) and whose
    /// `Origin`, when it sends one, is the origin of that same host.
    pub fn check(&self, host: Option<&str>, origin: Option<&str>) -> Result<(), Rejection> {
        let host = host
            .filter(|host| !host.is_empty())
            .ok_or(Rejection::MissingHost)?;
        let lower_host = host.to_ascii_lowercase();
        let local = self.is_local(&hostname_of(&lower_host));
        let proxied = self
            .public
            .as_ref()
            .is_some_and(|public| public.host() == lower_host);
        if !local && !proxied {
            return Err(Rejection::ForbiddenHost {
                host: host.to_owned(),
                hint: self.hint(),
            });
        }
        let Some(origin) = origin else { return Ok(()) };
        let configured = self
            .public
            .as_ref()
            .is_some_and(|public| public.as_str() == origin);
        // A public Host must not make another scheme or port on that origin trusted.
        let same_host = !proxied && origin_host(origin).is_some_and(|named| named == lower_host);
        match configured || same_host {
            true => Ok(()),
            false => Err(Rejection::ForbiddenOrigin {
                origin: origin.to_owned(),
                host: host.to_owned(),
                hint: self.hint(),
            }),
        }
    }

    fn is_local(&self, name: &str) -> bool {
        match self.loopback_only {
            true => LOOPBACK.contains(&name),
            false => name.parse::<IpAddr>().is_ok() || self.names.contains(name),
        }
    }

    /// Names the configured origin: it can differ from the rejected one only in the scheme,
    /// which makes an Origin and a Host that print identically look like a contradiction.
    fn hint(&self) -> String {
        match &self.public {
            Some(public) => format!("allowed origin is \"{public}\""),
            None => UNCONFIGURED_HINT.to_owned(),
        }
    }
}

/// The machine's hostname, its short form and its mDNS name, so LAN visitors can use any
/// of them.
#[must_use]
pub fn own_names() -> Vec<String> {
    let full = rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned();
    let short = full.split('.').next().unwrap_or(&full).to_owned();
    let mdns = format!("{short}{MDNS_SUFFIX}");
    vec![full, short, mdns]
}

/// The `Host` without its port; IPv6 literals lose their brackets.
fn hostname_of(host: &str) -> String {
    if let Some(inside) = host.strip_prefix('[') {
        return inside.split(']').next().unwrap_or(inside).to_owned();
    }
    // A bare IPv6 address is not a valid Host header; it is taken whole.
    match (host.parse::<IpAddr>().is_ok(), host.rsplit_once(':')) {
        (false, Some((name, _port))) => name.to_owned(),
        (true, _) | (false, None) => host.to_owned(),
    }
}

#[cfg(test)]
#[path = "request_guard_tests.rs"]
mod tests;
