//! Routing and merging language-server answers, including explicitly granted external files.
use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures::FutureExt;
use futures::future::BoxFuture;

use super::super::documents::Origin;
use super::super::merge::{merge_hovers, merge_locations, union};
use super::super::pool_server::PoolServer;
use super::LspPool;
use crate::error::{Error, Result};
use crate::protocol::{
    LspHoverResponse, LspLocationsResponse, LspOccurrencesResponse, LspPosition, LspSymbolsResponse, fuzzy_rank,
    language_of,
};

const SYMBOL_LIMIT: usize = 200;

#[derive(Clone, Copy)]
enum Locate {
    Definition,
    TypeDefinition,
    References,
}

impl LspPool {
    /// Reads an external file only if a running server named it.
    pub async fn read_external(&self, path: &str) -> Option<Vec<u8>> {
        for server in self.servers() {
            if let Some(contents) = server.bridge.read_external(path).await {
                return Some(contents);
            }
        }
        None
    }

    pub fn named(&self, path: &str) -> bool {
        self.servers().iter().any(|s| matches!(s.bridge.origin(path), Some(Origin::External { .. })))
    }

    pub async fn definition(&self, pos: &LspPosition) -> Result<LspLocationsResponse> {
        self.locations(pos, Locate::Definition).await
    }

    pub async fn type_definition(&self, pos: &LspPosition) -> Result<LspLocationsResponse> {
        self.locations(pos, Locate::TypeDefinition).await
    }

    pub async fn references(&self, pos: &LspPosition) -> Result<LspLocationsResponse> {
        self.locations(pos, Locate::References).await
    }

    /// Every server's hover, owner first; `owner_only` asks only the owner.
    pub async fn hover(&self, pos: &LspPosition, owner_only: bool) -> Result<LspHoverResponse> {
        let servers = self.servers_for(&pos.path)?;
        let asked = servers.iter().take(if owner_only { 1 } else { usize::MAX }).map(|s| s.bridge.hover(pos).boxed());
        union(asked.collect(), merge_hovers, |r: &LspHoverResponse| r.contents.is_none(), |r| &mut r.inconclusive).await
    }

    pub async fn occurrences(&self, pos: &LspPosition) -> Result<LspOccurrencesResponse> {
        self.owner_for(&pos.path)?.bridge.occurrences(pos).await
    }

    pub async fn document_symbols(&self, path: &str) -> Result<LspSymbolsResponse> {
        self.owner_for(path)?.bridge.document_symbols(path).await
    }

    /// Workspace symbols, ranked before applying the response cap.
    pub async fn workspace_symbols(&self, query: &str) -> Result<LspSymbolsResponse> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(Error::LspUnavailable("the language servers are shutting down".into()));
        }
        let servers = self.servers();
        if servers.is_empty() {
            return Err(Error::LspUnavailable("no language server is running".into()));
        }
        let asked = servers.iter().map(|s| s.bridge.workspace_symbols(query).boxed()).collect();
        let merge = |answers: Vec<LspSymbolsResponse>| {
            let all: Vec<_> = answers.into_iter().flat_map(|a| a.symbols).collect();
            let mut symbols = fuzzy_rank(&all, query, |s| s.name.as_str());
            symbols.truncate(SYMBOL_LIMIT);
            LspSymbolsResponse { symbols, inconclusive: None }
        };
        union(asked, merge, |r: &LspSymbolsResponse| r.symbols.is_empty(), |r| &mut r.inconclusive).await
    }

    /// Snapshot files go to owners then joiners; external files only to bridges that granted them.
    fn servers_for(&self, path: &str) -> Result<Vec<Arc<PoolServer>>> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(Error::LspUnavailable("the language servers are shutting down".into()));
        }
        let all = self.servers();
        let found: Vec<Arc<PoolServer>> = match language_of(path) {
            Some(language) => {
                let owners = all.iter().filter(|s| s.routes().languages.contains(&language));
                let joiners = all.iter().filter(|s| s.routes().joins.contains(&language));
                owners.chain(joiners).filter(|s| s.bridge.origin(path).is_some()).cloned().collect()
            }
            None => Vec::new(),
        };
        if found.is_empty() {
            return Err(Error::LspUnavailable(format!("no language server for {path}")));
        }
        Ok(found)
    }

    fn owner_for(&self, path: &str) -> Result<Arc<PoolServer>> {
        Ok(self.servers_for(path)?.remove(0))
    }

    async fn locations(&self, pos: &LspPosition, which: Locate) -> Result<LspLocationsResponse> {
        let servers = self.servers_for(&pos.path)?;
        let asked: Vec<BoxFuture<'_, Result<LspLocationsResponse>>> = servers
            .iter()
            .map(|s| match which {
                Locate::Definition => s.bridge.definition(pos).boxed(),
                Locate::TypeDefinition => s.bridge.type_definition(pos).boxed(),
                Locate::References => s.bridge.references(pos).boxed(),
            })
            .collect();
        union(asked, merge_locations, |r: &LspLocationsResponse| r.locations.is_empty(), |r| &mut r.inconclusive).await
    }
}
