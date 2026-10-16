//! Assembling and running the server: the guarded API, the guarded socket, the client build.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

use super::guard_layer::{guard_api, guard_socket};
use super::listening::bind::{PortChoice, bind};
use super::listening::server_url::ServerUrl;
use super::pages::client_dir::ClientDir;
use super::server_error::ServerError;
use super::ws_hub::WsHub;
use crate::domains::review::session::review_session::Session;
use crate::kernel::allowed_origin::AllowedOrigin;
use crate::kernel::request_guard::{RequestGuard, own_names};

/// How long a stopping server waits for connections to finish before it drops them.
const STOP_GRACE: Duration = Duration::from_secs(2);
const SOCKET_ROUTE: &str = "/ws";

pub struct ServerConfig {
    pub host: String,
    pub port: PortChoice,
    /// A public origin a reverse proxy serves this server under.
    pub allowed_origin: Option<AllowedOrigin>,
    pub client: ClientDir,
}

/// A server that is listening. Dropping it leaves the task running; [`stop`](Self::stop) ends it.
pub struct RunningServer {
    url: ServerUrl,
    clients: watch::Receiver<usize>,
    stop: watch::Sender<bool>,
    task: JoinHandle<io::Result<()>>,
}

/// Binds, mounts `api` behind the guard next to the socket and the client build, and serves.
pub async fn start(
    config: ServerConfig,
    api: Router,
    session: Arc<Session>,
) -> Result<RunningServer, ServerError> {
    let listener = bind(&config.host, config.port).await?;
    let addr = listener.local_addr().map_err(ServerError::Serve)?;
    let (stop, stopping) = watch::channel(false);
    let hub = WsHub::new(session, stopping.clone());
    let clients = hub.clients();
    let guard = Arc::new(RequestGuard::new(
        &config.host,
        &own_names(),
        config.allowed_origin,
    ));
    let app = assemble(&guard, api, hub, &config.client);
    let task = tokio::spawn(async move {
        let mut stopping = stopping;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                // A dropped sender also means stop: nothing is left to serve for.
                let _ = stopping.wait_for(|stop| *stop).await;
            })
            .await
    });
    Ok(RunningServer {
        url: ServerUrl::new(&config.host, addr),
        clients,
        stop,
        task,
    })
}

/// Everything under one router: the API and the socket behind the guard, and for any other
/// path the client build, falling back to its index so the page's own routes load.
pub(super) fn assemble(
    guard: &Arc<RequestGuard>,
    api: Router,
    hub: WsHub,
    client: &ClientDir,
) -> Router {
    let api = api.layer(from_fn_with_state(Arc::clone(guard), guard_api));
    let socket = Router::new()
        .route(SOCKET_ROUTE, get(WsHub::upgrade))
        .layer(from_fn_with_state(Arc::clone(guard), guard_socket))
        .with_state(hub);
    let files = ServeDir::new(client.path()).fallback(ServeFile::new(client.index()));
    Router::new()
        .merge(api)
        .merge(socket)
        .fallback_service(files)
        .layer(CompressionLayer::new())
}

impl RunningServer {
    #[must_use]
    pub const fn url(&self) -> &ServerUrl {
        &self.url
    }

    /// How many pages are connected, as it changes.
    #[must_use]
    pub fn clients(&self) -> watch::Receiver<usize> {
        self.clients.clone()
    }

    /// Resolves only if the server ends on its own, with why. Never otherwise.
    pub async fn failed(&mut self) -> ServerError {
        match (&mut self.task).await {
            Ok(Ok(())) => std::future::pending().await,
            Ok(Err(error)) => ServerError::Serve(error),
            Err(error) => ServerError::Serve(io::Error::other(error)),
        }
    }

    /// Closes every page's socket, finishes the requests in flight, and stops listening.
    pub async fn stop(mut self) {
        self.stop.send_replace(true);
        if tokio::time::timeout(STOP_GRACE, &mut self.task)
            .await
            .is_err()
        {
            self.task.abort();
        }
    }
}
