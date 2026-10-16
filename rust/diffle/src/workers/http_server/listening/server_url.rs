//! Where the running server can be reached.

use std::fmt;
use std::net::SocketAddr;

/// Bound to every interface, a browser still has to be sent somewhere it can reach.
const ANY_V4: &str = "0.0.0.0";
const ANY_V6: &str = "::";
const LOCAL_NAME: &str = "localhost";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerUrl {
    host: String,
    port: u16,
}

impl ServerUrl {
    /// The host the server was asked to bind, and the port it got.
    #[must_use]
    pub fn new(host: &str, addr: SocketAddr) -> Self {
        Self {
            host: host.to_owned(),
            port: addr.port(),
        }
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// The address for a browser on this machine: `localhost` where the server listens on
    /// every interface.
    #[must_use]
    pub fn for_this_machine(&self) -> Self {
        match self.host.as_str() {
            ANY_V4 | ANY_V6 => Self {
                host: LOCAL_NAME.to_owned(),
                port: self.port,
            },
            _ => self.clone(),
        }
    }
}

impl fmt::Display for ServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.host.contains(':') {
            true => write!(f, "http://[{}]:{}/", self.host, self.port),
            false => write!(f, "http://{}:{}/", self.host, self.port),
        }
    }
}

#[cfg(test)]
#[path = "server_url_tests.rs"]
mod tests;
