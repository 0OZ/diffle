//! One review, served: resolve it, listen, open the browser, wait for its end, hand the
//! comments over. Each step is a call; the rules live in the parts.

use std::sync::Arc;

use super::ending::{Ending, wait_for_ending};
use super::hand_off::hand_off;
use super::startup_report::report_lines;
use crate::app::app_error::AppError;
use crate::app::config::Config;
use crate::domains::repository::git_repo::GitRepo;
use crate::domains::review::api::api_state::ApiState;
use crate::domains::review::api::router_api::router;
use crate::domains::review::mode::comparison_label::comparison_label;
use crate::domains::review::session::review_session::{Session, SessionConfig};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::settings::config_store::ConfigStore;
use crate::workers::browser_opener::open_in_browser;
use crate::workers::http_server::pages::browser_lifetime::CLOSE_GRACE;
use crate::workers::http_server::pages::client_dir::ClientDir;
use crate::workers::http_server::server::{RunningServer, ServerConfig, start};
use crate::workers::interrupt::Interrupts;
use crate::workers::session_watchers::session_watchers;

pub async fn serve_review(config: Config, settings: Arc<ConfigStore>) -> Result<(), AppError> {
    let repo = Arc::new(GitRepo::open(config.directory()).await?);
    let stored = settings.get();
    let session = Session::new(
        Arc::clone(&repo),
        SessionConfig {
            context: config.context().unwrap_or(stored.context_lines),
            follow_refs: stored.follow_refs,
            watcher: config.watch().then(session_watchers),
        },
    );
    // Bad input ends the run here, before a port is taken or a browser is opened.
    let comparison = session.resolve(config.request()).await?;
    let client = ClientDir::locate(config.client_dir())?;
    let mut interrupts = Interrupts::listen().map_err(AppError::Signals)?;

    let api = router(ApiState::new(
        Arc::clone(&session),
        settings,
        config.auto_viewed().to_vec(),
    ));
    let mut server = start(
        ServerConfig {
            host: config.host().to_owned(),
            port: config.port(),
            allowed_origin: config.allowed_origin().cloned(),
            client,
        },
        api,
        Arc::clone(&session),
    )
    .await?;
    if config.browser().opens() && !open_in_browser(&server.url().for_this_machine().to_string()) {
        tracing::warn!("could not open a browser; open the address above yourself");
    }
    tracing::info!("🚀 diffle running at {}", server.url());
    tracing::info!("📂 repo {}", repo.root().display());

    // The first snapshot is computed while the page loads; a signal during it still ends the run.
    let started = tokio::select! {
        started = session.start(comparison) => Some(started),
        () = interrupts.wait() => None,
    };
    let ending = match started {
        None => Ending::Interrupted,
        Some(Err(error)) => {
            dispose(&session, server).await;
            return Err(error.into());
        }
        Some(Ok(snapshot)) => {
            report(&config, &session, &snapshot);
            wait_for_ending(
                &mut interrupts,
                config.browser(),
                server.clients(),
                CLOSE_GRACE,
                server.failed(),
            )
            .await
        }
    };
    finish(ending, &session, server).await
}

fn report(config: &Config, session: &Session, snapshot: &Snapshot) {
    let Ok(active) = session.active() else {
        return;
    };
    let parts = snapshot.parts();
    let comparison = active.mode().comparison();
    for line in report_lines(
        &comparison_label(comparison),
        &parts.changed,
        comparison.live(),
        session.follows(&active),
        config.watch(),
    ) {
        tracing::info!("{line}");
    }
}

async fn finish(ending: Ending, session: &Session, server: RunningServer) -> Result<(), AppError> {
    match ending {
        Ending::Interrupted | Ending::LastTabClosed => {
            tracing::info!("👋 shutting down");
            hand_off(session).await;
            dispose(session, server).await;
            Ok(())
        }
        Ending::ServerFailed(error) => {
            dispose(session, server).await;
            Err(error.into())
        }
    }
}

/// Ends the watch, then the sockets, so nothing recomputes for a page that is gone.
async fn dispose(session: &Session, server: RunningServer) {
    session.close().await;
    server.stop().await;
}
