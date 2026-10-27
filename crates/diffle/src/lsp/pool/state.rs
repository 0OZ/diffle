//! Pool state and coalesced status publication.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use indexmap::IndexMap;

use super::super::documents::ReadFn;
use super::super::locator::HasFn;
use super::super::pool_server::PoolServer;
use super::super::registry::Lookup;
use crate::protocol::{LanguageId, LspLogEntry, LspMissing, LspStatus};

/// Status bursts ride one trailing publish after an immediate leading update.
const PUBLISH_WINDOW: Duration = Duration::from_millis(20);

pub struct PoolOptions {
    /// Repository root; every server's workspace folder and cwd.
    pub root: std::path::PathBuf,
    pub read: ReadFn,
    pub has: HasFn,
    /// The whole pool's status, coalesced.
    pub on_status: Arc<dyn Fn(LspStatus) + Send + Sync>,
    /// Command per language, replacing the built-in candidates. `""` turns the language off.
    pub overrides: HashMap<LanguageId, String>,
    /// Languages requested on the command line, also started when the diff has none.
    pub preload: Vec<LanguageId>,
    pub lookup: Lookup,
    pub max_open: Option<usize>,
}

#[derive(Default)]
struct Publish {
    pending: bool,
    due: bool,
}

/// Routes files to their language servers and merges their answers. A lifecycle lease serializes
/// tracking, reset, and shutdown; session read leases keep queries clear of owned disk mutations.
pub struct LspPool {
    pub(super) opts: PoolOptions,
    pub(super) entries: Mutex<IndexMap<String, Arc<PoolServer>>>,
    pub(super) missing: Mutex<Vec<LspMissing>>,
    pub(super) resolved: Mutex<HashSet<LanguageId>>,
    pub(super) closing: AtomicBool,
    pub(super) stopping: tokio::sync::watch::Sender<bool>,
    pub(super) lifecycle: tokio::sync::Mutex<()>,
    publish: Mutex<Publish>,
}

impl LspPool {
    pub(super) fn empty(opts: PoolOptions) -> Arc<Self> {
        Arc::new(Self {
            opts,
            entries: Mutex::default(),
            missing: Mutex::default(),
            resolved: Mutex::default(),
            closing: AtomicBool::new(false),
            stopping: tokio::sync::watch::channel(false).0,
            lifecycle: tokio::sync::Mutex::new(()),
            publish: Mutex::default(),
        })
    }

    pub fn status(&self) -> LspStatus {
        let servers = self.servers().iter().map(|s| s.status()).collect();
        LspStatus { enabled: true, servers, missing: self.missing.lock().unwrap().clone() }
    }

    pub(super) fn servers(&self) -> Vec<Arc<PoolServer>> {
        self.entries.lock().unwrap().values().cloned().collect()
    }

    /// The server started by `command`, if one was.
    pub fn server(&self, command: &str) -> Option<Arc<PoolServer>> {
        self.entries.lock().unwrap().get(command).cloned()
    }

    /// Retained log lines of the server started by `command`.
    pub fn log(&self, command: &str) -> Option<Vec<LspLogEntry>> {
        self.entries.lock().unwrap().get(command).map(|s| s.bridge.entries())
    }

    pub(super) fn publish(self: &Arc<Self>) {
        if self.closing.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut publish = self.publish.lock().unwrap();
            if publish.pending {
                publish.due = true;
                return;
            }
            publish.pending = true;
        }
        (self.opts.on_status)(self.status());
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::time::sleep(PUBLISH_WINDOW).await;
            let Some(pool) = weak.upgrade() else { return };
            let due = {
                let mut publish = pool.publish.lock().unwrap();
                publish.pending = false;
                std::mem::take(&mut publish.due)
            };
            if due {
                pool.publish();
            }
        });
    }
}
