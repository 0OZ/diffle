//! The whole server over real sockets: the page, the guarded API, and the live socket.

use std::time::Duration;

use futures::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::server_fixtures::Running;
use crate::domains::review::session::session_events::SessionEvent;

const PATIENCE: Duration = Duration::from_secs(10);
const BRIEFLY: Duration = Duration::from_millis(200);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn connect(running: &Running) -> Socket {
    let url = format!("ws://127.0.0.1:{}/ws", running.port());
    connect_async(url).await.unwrap().0
}

async fn next_text(socket: &mut Socket) -> String {
    loop {
        match tokio::time::timeout(PATIENCE, socket.next()).await.unwrap() {
            Some(Ok(Message::Text(text))) => return text.to_string(),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            other => panic!("expected text, got {other:?}"),
        }
    }
}

async fn clients(running: &Running, wanted: usize) {
    let mut count = running.server.clients();
    tokio::time::timeout(PATIENCE, count.wait_for(|now| *now == wanted))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn the_client_build_is_served_and_unknown_paths_load_its_index() {
    let running = Running::start().await;
    let index = running.get("/").await;
    assert_eq!(index.status, 200);
    assert!(index.text().contains("<title>diffle</title>"));
    assert_eq!(index.headers["content-type"], "text/html");

    let script = running.get("/assets/app.js").await;
    assert_eq!(script.status, 200);
    assert!(script.headers["content-type"].contains("javascript"));

    let page_route = running.get("/some/page/route").await;
    assert_eq!(page_route.status, 200);
    assert_eq!(page_route.body, index.body);
}

#[tokio::test]
async fn responses_are_compressed_for_a_client_that_accepts_it() {
    let running = Running::start().await;
    let reply = running
        .request("GET", "/assets/app.js", &[("Accept-Encoding", "gzip")])
        .await;
    assert_eq!(reply.headers["content-encoding"], "gzip");
    assert!(reply.body.len() < 1200, "{} bytes", reply.body.len());
}

#[tokio::test]
async fn the_api_answers_over_the_socket() {
    let running = Running::start().await;
    let reply = running.get("/api/snapshot").await;
    assert_eq!(reply.status, 200);
    let snapshot: Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(snapshot["version"], json!(1));
    assert_eq!(snapshot["changed"][0]["path"], json!("b.txt"));
}

#[tokio::test]
async fn an_unknown_api_path_is_json_not_the_page() {
    let running = Running::start().await;
    let reply = running.get("/api/nope").await;
    assert_eq!(reply.status, 404);
    assert_eq!(reply.headers["content-type"], "application/json");
}

#[tokio::test]
async fn the_api_turns_away_a_foreign_host_and_a_foreign_origin_but_the_page_does_not() {
    let running = Running::start().await;
    let host = running
        .request("GET", "/api/snapshot", &[("Host", "evil.example")])
        .await;
    assert_eq!(host.status, 403);
    let error: Value = serde_json::from_slice(&host.body).unwrap();
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .starts_with("forbidden Host \"evil.example\""),
        "{error}"
    );

    let origin = running
        .request(
            "GET",
            "/api/snapshot",
            &[("Origin", "https://evil.example")],
        )
        .await;
    assert_eq!(origin.status, 403);
    assert!(
        origin.text().contains("forbidden Origin"),
        "{}",
        origin.text()
    );

    let page = running
        .request("GET", "/", &[("Host", "evil.example")])
        .await;
    assert_eq!(
        page.status, 200,
        "the page holds nothing the API does not guard"
    );
}

#[tokio::test]
async fn a_configured_public_origin_is_trusted() {
    use crate::kernel::allowed_origin::AllowedOrigin;
    let origin = AllowedOrigin::parse("https://review.example.com").unwrap();
    let running = Running::start_with(Some(origin)).await;
    let reply = running
        .request(
            "GET",
            "/api/snapshot",
            &[
                ("Host", "review.example.com"),
                ("Origin", "https://review.example.com"),
            ],
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
}

#[tokio::test]
async fn a_connected_page_hears_what_the_session_announces() {
    let running = Running::start().await;
    let mut socket = connect(&running).await;
    clients(&running, 1).await;
    running.fixture.session.announce(SessionEvent::Threads);
    assert_eq!(next_text(&mut socket).await, r#"{"type":"threads"}"#);
    running.fixture.session.announce(SessionEvent::Moved(None));
    assert_eq!(
        next_text(&mut socket).await,
        r#"{"type":"moved","moved":null}"#
    );
}

#[tokio::test]
async fn every_connected_page_hears_each_message() {
    let running = Running::start().await;
    let mut first = connect(&running).await;
    let mut second = connect(&running).await;
    clients(&running, 2).await;
    running.fixture.session.announce(SessionEvent::Viewed);
    assert_eq!(next_text(&mut first).await, r#"{"type":"viewed"}"#);
    assert_eq!(next_text(&mut second).await, r#"{"type":"viewed"}"#);
}

#[tokio::test]
async fn closing_a_page_updates_the_count() {
    let running = Running::start().await;
    let mut socket = connect(&running).await;
    clients(&running, 1).await;
    socket.close(None).await.unwrap();
    clients(&running, 0).await;
}

#[tokio::test]
async fn the_socket_rejects_a_foreign_origin_naming_the_header() {
    let running = Running::start().await;
    let mut request = format!("ws://127.0.0.1:{}/ws", running.port())
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "https://evil.example".parse().unwrap());
    let error = connect_async(request).await.unwrap_err();
    let tokio_tungstenite::tungstenite::Error::Http(response) = error else {
        panic!("expected an HTTP rejection, got {error:?}");
    };
    assert_eq!(response.status(), 403);
    let body = String::from_utf8_lossy(response.body().as_deref().unwrap_or_default()).into_owned();
    assert!(body.contains("forbidden Origin"), "{body}");
}

#[tokio::test]
async fn a_page_that_sends_too_much_is_dropped() {
    let running = Running::start().await;
    let mut socket = connect(&running).await;
    clients(&running, 1).await;
    let oversized = Message::text("x".repeat(8192));
    let _ = futures::SinkExt::send(&mut socket, oversized).await;
    clients(&running, 0).await;
}

#[tokio::test]
async fn stopping_closes_the_pages_and_ends_the_server() {
    let running = Running::start().await;
    let mut socket = connect(&running).await;
    clients(&running, 1).await;
    let port = running.port();
    let Running { server, .. } = running;
    tokio::time::timeout(PATIENCE, server.stop()).await.unwrap();
    let closed = tokio::time::timeout(PATIENCE, socket.next()).await.unwrap();
    assert!(
        matches!(closed, None | Some(Ok(Message::Close(_))) | Some(Err(_))),
        "{closed:?}"
    );
    assert!(
        tokio::time::timeout(BRIEFLY, TcpStream::connect(("127.0.0.1", port)))
            .await
            .map_or(true, |attempt| attempt.is_err()),
        "nothing listens any more"
    );
}

#[tokio::test]
async fn a_page_that_fell_behind_is_told_to_refetch_everything() {
    let running = Running::start().await;
    let (_, stopping) = tokio::sync::watch::channel(false);
    let hub = super::ws_hub::WsHub::new(std::sync::Arc::clone(&running.fixture.session), stopping);
    let messages: Vec<Value> = hub
        .catch_up()
        .iter()
        .map(|message| serde_json::to_value(message).unwrap())
        .collect();
    assert_eq!(
        messages,
        [
            json!({"type": "snapshot", "version": 1}),
            json!({"type": "threads"}),
            json!({"type": "viewed"}),
            json!({"type": "config"}),
        ]
    );
}
