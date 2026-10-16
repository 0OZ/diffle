//! `/ws`: pushes what the session announces to every connected page. Pages send nothing, so
//! anything but a close or a ping is ignored and anything large is not a page.

use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, watch};

use crate::domains::review::api::wire::server_message_wire::ServerMessageWire;
use crate::domains::review::session::review_session::Session;
use crate::domains::review::session::session_events::SessionEvent;

/// Pages send nothing; a frame larger than a ping is dropped along with its sender.
const MAX_PAYLOAD_BYTES: usize = 4096;

#[derive(Clone)]
pub struct WsHub {
    session: Arc<Session>,
    clients: Arc<watch::Sender<usize>>,
    stopping: watch::Receiver<bool>,
}

/// Counts a connected page for as long as it is held.
struct Presence(Arc<watch::Sender<usize>>);

impl Presence {
    fn arrive(clients: &Arc<watch::Sender<usize>>) -> Self {
        clients.send_modify(|count| *count = count.saturating_add(1));
        Self(Arc::clone(clients))
    }
}

impl Drop for Presence {
    fn drop(&mut self) {
        self.0.send_modify(|count| *count = count.saturating_sub(1));
    }
}

impl WsHub {
    #[must_use]
    pub fn new(session: Arc<Session>, stopping: watch::Receiver<bool>) -> Self {
        Self {
            session,
            clients: Arc::new(watch::channel(0).0),
            stopping,
        }
    }

    /// How many pages are connected, as it changes.
    #[must_use]
    pub fn clients(&self) -> watch::Receiver<usize> {
        self.clients.subscribe()
    }

    /// The `/ws` route.
    pub async fn upgrade(State(hub): State<Self>, upgrade: WebSocketUpgrade) -> Response {
        // Subscribed before the handshake answers: whatever the session announces from here on
        // reaches this page, even if it asks for the snapshot first.
        let events = hub.session.subscribe();
        upgrade
            .max_message_size(MAX_PAYLOAD_BYTES)
            .max_frame_size(MAX_PAYLOAD_BYTES)
            .on_upgrade(move |socket| hub.serve(socket, events))
    }

    async fn serve(self, mut socket: WebSocket, mut events: broadcast::Receiver<SessionEvent>) {
        let _present = Presence::arrive(&self.clients);
        let mut stopping = self.stopping.clone();
        loop {
            let pushed = tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => push(&mut socket, &[ServerMessageWire::from(&event)]).await,
                    Err(RecvError::Lagged(_)) => push(&mut socket, &self.catch_up()).await,
                    Err(RecvError::Closed) => false,
                },
                incoming = socket.recv() => matches!(incoming, Some(Ok(message)) if !matches!(message, Message::Close(_))),
                _ = stopping.changed() => false,
            };
            if !pushed {
                break;
            }
        }
        // The page may be gone already; a close frame it never reads costs nothing.
        let _ = socket.send(Message::Close(None)).await;
    }

    /// A page that fell behind missed events it cannot name; every event only says "refetch",
    /// so tell it to refetch everything.
    pub(super) fn catch_up(&self) -> Vec<ServerMessageWire> {
        let version = self
            .session
            .follow_snapshots()
            .borrow()
            .as_ref()
            .map(|snapshot| snapshot.version().get());
        version
            .map(|version| ServerMessageWire::Snapshot { version })
            .into_iter()
            .chain([
                ServerMessageWire::Threads,
                ServerMessageWire::Viewed,
                ServerMessageWire::Config,
            ])
            .collect()
    }
}

/// Whether the page is still there to hear more.
async fn push(socket: &mut WebSocket, messages: &[ServerMessageWire]) -> bool {
    for message in messages {
        let Ok(text) = serde_json::to_string(message) else {
            tracing::error!(?message, "a server message did not serialize");
            continue;
        };
        if socket.send(Message::text(text)).await.is_err() {
            return false;
        }
    }
    true
}
