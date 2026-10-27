//! Starting, tracking, resetting and reaping servers, serialized against shutdown.
use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures::FutureExt;

use super::super::pool_server::{PathsByLanguage, PoolServer, Routes, ServerOptions, paths_of};
use super::super::registry::resolve_servers;
use super::LspPool;
use super::PoolOptions;
use crate::protocol::{ChangeStatus, LanguageId, Snapshot, follows_checkout, language_of};
use crate::session::Session;

impl LspPool {
    pub fn new(opts: PoolOptions) -> Arc<Self> {
        let pool = Self::empty(opts);
        pool.ensure(&pool.preload());
        pool
    }

    /// Isolated roots have not been checked out yet: preload waits for their first coherent snapshot.
    pub fn deferred(opts: PoolOptions) -> Arc<Self> {
        Self::empty(opts)
    }

    /// Wires the session's mutation boundary and version-checked tracking before its first activation.
    pub fn attach(self: &Arc<Self>, session: &Arc<Session>) {
        if session.repo.is_isolated() {
            let pool = Arc::downgrade(self);
            session.before_checkout(move || {
                let pool = pool.upgrade();
                async move {
                    if let Some(pool) = pool {
                        pool.reset().await;
                    }
                }
                .boxed()
            });
            let pool = Arc::downgrade(self);
            session.on_checkout_restored(move |snap| {
                let pool = pool.upgrade();
                async move {
                    if let Some(pool) = pool {
                        pool.track(&snap).await;
                    }
                }
                .boxed()
            });
        }
        let (tracking, owner) = (Arc::downgrade(self), Arc::downgrade(session));
        session.on_snapshot(move |snap| {
            let (Some(pool), Some(session)) = (tracking.upgrade(), owner.upgrade()) else { return };
            session.track_snapshot(snap.clone(), move |snap| async move { pool.track(&snap).await }.boxed());
        });
    }

    fn preload(&self) -> PathsByLanguage {
        self.opts.preload.iter().map(|l| (*l, Vec::new())).collect()
    }

    /// Tracks only checkout-coherent text. The same-revision process keeps its warm index.
    pub async fn track(self: &Arc<Self>, snap: &Snapshot) {
        let _guard = self.lifecycle.lock().await;
        if self.closing.load(Ordering::SeqCst) {
            return;
        }
        let mut stopping = self.stopping.subscribe();
        tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => {}
            () = self.track_current(snap) => {}
        }
    }

    async fn track_current(self: &Arc<Self>, snap: &Snapshot) {
        let mut by_language = PathsByLanguage::new();
        if follows_checkout(snap) {
            by_language = self.preload();
            for f in snap.changed.iter().filter(|f| f.status != ChangeStatus::D && !f.binary) {
                if let Some(language) = language_of(&f.path) {
                    by_language.entry(language).or_default().push(f.path.clone());
                }
            }
        }
        self.ensure(&by_language);
        let servers = self.servers();
        let passes = servers.iter().map(|s| {
            let tracked = paths_of(&s.routes(), &by_language);
            s.follow(&tracked);
            async move { s.bridge.track(&tracked).await }
        });
        futures::future::join_all(passes).await;
    }

    /// Reaps and removes old bridges: document caches, external grants and project catalogs all die with them.
    pub async fn reset(self: &Arc<Self>) {
        let _guard = self.lifecycle.lock().await;
        let servers = self.servers();
        futures::future::join_all(servers.iter().map(|s| async move { s.bridge.close().await })).await;
        self.entries.lock().unwrap().clear();
        self.resolved.lock().unwrap().clear();
        self.missing.lock().unwrap().clear();
        self.publish();
    }

    /// Interrupts tracking and reaps every child without waiting behind an initialization lease.
    pub async fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.stopping.send_replace(true);
        // spawn holds entries while checking closing and registering a child, so this cannot miss one.
        let servers = self.servers();
        futures::future::join_all(servers.iter().map(|s| async move { s.bridge.close().await })).await;
    }

    fn ensure(self: &Arc<Self>, by_language: &PathsByLanguage) {
        if self.closing.load(Ordering::SeqCst) {
            return;
        }
        let fresh: Vec<LanguageId> = {
            let mut resolved = self.resolved.lock().unwrap();
            by_language.keys().copied().filter(|l| resolved.insert(*l)).collect()
        };
        if fresh.is_empty() {
            return;
        }
        let resolution = resolve_servers(fresh, &self.opts.overrides, &self.opts.lookup);
        let changed = !resolution.servers.is_empty() || !resolution.missing.is_empty();
        self.missing.lock().unwrap().extend(resolution.missing);
        for server in resolution.servers {
            let existing = self.entries.lock().unwrap().get(&server.command).cloned();
            let entry = match existing {
                Some(entry) => entry,
                None => {
                    let routes = Routes { languages: server.languages.clone(), joins: server.joins.clone() };
                    let Some(entry) = self.spawn(&server.command, &paths_of(&routes, by_language)) else { return };
                    entry
                }
            };
            let mut routes = entry.routes.lock().unwrap();
            for l in server.languages {
                if !routes.languages.contains(&l) {
                    routes.languages.push(l);
                }
            }
            for l in server.joins {
                if !routes.joins.contains(&l) {
                    routes.joins.push(l);
                }
            }
        }
        if changed {
            self.publish();
        }
    }

    fn spawn(self: &Arc<Self>, command: &str, paths: &[String]) -> Option<Arc<PoolServer>> {
        let weak = Arc::downgrade(self);
        let opts = ServerOptions {
            root: self.opts.root.clone(),
            read: self.opts.read.clone(),
            has: self.opts.has.clone(),
            lookup: self.opts.lookup.clone(),
            max_open: self.opts.max_open,
            on_status: Arc::new(move || {
                if let Some(pool) = weak.upgrade() {
                    pool.publish();
                }
            }),
        };
        let mut entries = self.entries.lock().unwrap();
        if self.closing.load(Ordering::SeqCst) {
            return None;
        }
        let server = Arc::new(PoolServer::new(command, paths, opts));
        entries.insert(command.to_owned(), server.clone());
        Some(server)
    }
}
