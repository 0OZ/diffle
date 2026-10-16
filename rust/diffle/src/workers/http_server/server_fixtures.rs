//! A real server on a real port over a real repository, and a bare HTTP client to call it.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use super::listening::bind::PortChoice;
use super::pages::client_dir::ClientDir;
use super::server::{RunningServer, ServerConfig, start};
use crate::domains::review::api::api_fixtures::Fixture;
use crate::domains::review::api::router_api::router;
use crate::kernel::allowed_origin::AllowedOrigin;

const LOOPBACK: &str = "127.0.0.1";
const INDEX: &str = "<!doctype html><title>diffle</title><div id=root></div>";

/// What a request got back: the status line, the headers (names lower-cased), the body.
pub struct Reply {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub struct Running {
    pub server: RunningServer,
    pub fixture: Fixture,
    _client: tempfile::TempDir,
}

impl Running {
    /// The first commit's review (`HEAD^!`) behind a server with a small client build:
    /// `index.html` and `assets/app.js`.
    pub async fn start() -> Self {
        Self::start_with(None).await
    }

    pub async fn start_with(allowed_origin: Option<AllowedOrigin>) -> Self {
        let fixture = Fixture::opened(&["HEAD^!"]).await;
        let client = tempfile::tempdir().unwrap();
        std::fs::write(client.path().join("index.html"), INDEX.repeat(40)).unwrap();
        std::fs::create_dir(client.path().join("assets")).unwrap();
        std::fs::write(
            client.path().join("assets/app.js"),
            "console.log(1)\n".repeat(80),
        )
        .unwrap();
        let config = ServerConfig {
            host: LOOPBACK.to_owned(),
            port: PortChoice::Exact(0),
            allowed_origin,
            client: ClientDir::locate(Some(client.path())).unwrap(),
        };
        let api = router(fixture.state.clone());
        let server = start(config, api, std::sync::Arc::clone(&fixture.session))
            .await
            .unwrap();
        Self {
            server,
            fixture,
            _client: client,
        }
    }

    pub fn port(&self) -> u16 {
        self.server.url().port()
    }

    pub async fn get(&self, path: &str) -> Reply {
        self.request("GET", path, &[]).await
    }

    /// `count` sequential `GET`s of `path` on one kept-alive connection, and how long they took.
    /// A reply held back by Nagle's algorithm shows up here as about 40 ms per request.
    pub async fn time_kept_alive_gets(&self, path: &str, count: usize) -> Duration {
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {LOOPBACK}:{}\r\nAccept-Encoding: identity\r\n\r\n",
            self.port()
        );
        let stream = TcpStream::connect((LOOPBACK, self.port())).await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut read = BufReader::new(read);
        let started = Instant::now();
        for _ in 0..count {
            write.write_all(request.as_bytes()).await.unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert!(
                    read.read_line(&mut line).await.unwrap() > 0,
                    "connection closed"
                );
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
            }
            read.read_exact(&mut vec![0; length]).await.unwrap();
        }
        started.elapsed()
    }

    /// One HTTP/1.1 request on its own connection, read to the end. The body is the raw bytes
    /// the server sent, so ask for `identity` encoding when it is to be read as text.
    pub async fn request(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> Reply {
        try_request(self.port(), method, path, headers)
            .await
            .expect("the server answers")
    }
}

/// [`Running::request`] against any port; `None` when nothing there sends a response. Another
/// server may have taken the port number of one that stopped, so a connection alone says little.
pub async fn try_request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> Option<Reply> {
    let mut text = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    let host = format!("{LOOPBACK}:{port}");
    let sends = |name: &str| {
        headers
            .iter()
            .any(|(sent, _)| sent.eq_ignore_ascii_case(name))
    };
    if !sends("host") {
        text.push_str(&format!("Host: {host}\r\n"));
    }
    if !sends("accept-encoding") {
        text.push_str("Accept-Encoding: identity\r\n");
    }
    for (name, value) in headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    let mut stream = TcpStream::connect((LOOPBACK, port)).await.ok()?;
    stream.write_all(text.as_bytes()).await.ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.ok()?;
    parse(&raw)
}

fn parse(raw: &[u8]) -> Option<Reply> {
    let split = raw.windows(4).position(|window| window == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Some(Reply {
        status,
        headers,
        body: raw[split + 4..].to_vec(),
    })
}
