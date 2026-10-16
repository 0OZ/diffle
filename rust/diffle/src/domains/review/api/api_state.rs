//! What every handler shares: the session, the user's config and the run's own additions.

use std::sync::Arc;

use super::error_api::ApiError;
use crate::domains::review::review_error::ReviewError;
use crate::domains::review::session::review_session::{Active, Session};
use crate::domains::review::snapshot::review_snapshot::Snapshot;
use crate::domains::settings::config_store::ConfigStore;
use crate::domains::settings::user_config_wire::UserConfigWire;

#[derive(Clone)]
pub struct ApiState {
    session: Arc<Session>,
    config: Arc<ConfigStore>,
    /// Session-only additions from `--auto-viewed`.
    extra_auto_viewed: Arc<[String]>,
}

impl ApiState {
    #[must_use]
    pub fn new(
        session: Arc<Session>,
        config: Arc<ConfigStore>,
        extra_auto_viewed: Vec<String>,
    ) -> Self {
        Self {
            session,
            config,
            extra_auto_viewed: extra_auto_viewed.into(),
        }
    }

    #[must_use]
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

    #[must_use]
    pub fn config(&self) -> &Arc<ConfigStore> {
        &self.config
    }

    /// Waits until the first review is open: the server listens before it has computed one.
    pub async fn until_open(&self) -> Result<(), ApiError> {
        let mut latest = self.session.follow_snapshots();
        latest
            .wait_for(|snapshot| snapshot.is_some())
            .await
            .map(drop)
            .map_err(|_| ApiError::from(ReviewError::Interrupted))
    }

    /// The open review and its current snapshot.
    pub async fn current(&self) -> Result<(Arc<Active>, Arc<Snapshot>), ApiError> {
        let active = self.session.active()?;
        let snapshot = active.snapshotter().current().await?;
        Ok((active, snapshot))
    }

    /// The stored config plus this run's additions: `--auto-viewed` globs and the live context.
    #[must_use]
    pub fn effective_config(&self) -> UserConfigWire {
        UserConfigWire::effective(
            &self.config.get(),
            &self.extra_auto_viewed,
            self.session.context(),
        )
    }
}
