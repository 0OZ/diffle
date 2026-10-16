//! Why the server could not start or stopped.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("could not listen on {host}:{port}: {source}")]
    Bind {
        host: String,
        port: u16,
        source: io::Error,
    },
    #[error("port {port} on {host} is already in use; pick another with --port")]
    PortTaken { host: String, port: u16 },
    #[error("no free port from {first} to {last} on {host}; free one or pass --port")]
    NoFreePort { host: String, first: u16, last: u16 },
    #[error("the server stopped: {0}")]
    Serve(#[source] io::Error),
    #[error(
        "no client build at {dir}: build it with `npm run build`, or point --client-dir at one"
    )]
    NoClientBuild { dir: PathBuf },
}
