//! Taking a port: the default one with a fallback, or exactly the one asked for.

use std::io::ErrorKind;

use tokio::net::TcpListener;

use crate::workers::http_server::server_error::ServerError;

/// Unassigned and memorable.
pub const DEFAULT_PORT: u16 = 4966;
/// How many ports above the default are tried when it is taken.
const PROBE_PORTS: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortChoice {
    /// The default port, or the first free one above it: two reviews can run side by side.
    Probe,
    /// This port or failure; `0` lets the system pick.
    Exact(u16),
}

impl PortChoice {
    const fn range(self) -> (u16, u16) {
        match self {
            Self::Probe => (DEFAULT_PORT, DEFAULT_PORT.saturating_add(PROBE_PORTS)),
            Self::Exact(port) => (port, port),
        }
    }
}

/// Listens on `host` at the port `choice` allows. Only "address in use" moves on to the next
/// port; any other failure (a bad host, no permission) is the answer.
pub async fn bind(host: &str, choice: PortChoice) -> Result<TcpListener, ServerError> {
    let (first, last) = choice.range();
    for port in first..=last {
        match TcpListener::bind((host, port)).await {
            Ok(listener) => return Ok(listener),
            Err(error) if error.kind() == ErrorKind::AddrInUse => {}
            Err(source) => {
                return Err(ServerError::Bind {
                    host: host.to_owned(),
                    port,
                    source,
                });
            }
        }
    }
    Err(match choice {
        PortChoice::Probe => ServerError::NoFreePort {
            host: host.to_owned(),
            first,
            last,
        },
        PortChoice::Exact(port) => ServerError::PortTaken {
            host: host.to_owned(),
            port,
        },
    })
}

#[cfg(test)]
#[path = "bind_tests.rs"]
mod tests;
