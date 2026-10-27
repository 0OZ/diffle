//! Startup settlement and shutdown: close cannot outrun a spawn or wait on initialize.
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::capabilities::truthy;
use super::child::{self, command, signal, spawn_settled};
use super::{LspProcess, ProcessOptions};
use crate::lsp::rpc::{Connection, RequestError};
use crate::lsp::shapes::{all_features, client_capabilities, provider_of};
use crate::protocol::{LspFeature, LspState};

const INIT_TIMEOUT: Duration = Duration::from_secs(60);
const SHUTDOWN_RPC: Duration = Duration::from_millis(500);
const KILL_AFTER: Duration = Duration::from_millis(500);

impl LspProcess {
    /// Interrupts startup and returns only once a spawned child has been reaped.
    pub async fn close(&self) {
        let _shutdown = self.shutdown.lock().await;
        self.state.lock().unwrap().closing = true;
        self.ready.send_replace(Some(false));
        let mut started = self.started.subscribe();
        if let Err(error) = started.wait_for(|started| *started).await {
            crate::status!("[diffle] language server startup channel closed: {error}");
            return;
        }
        let (rpc, pid, initialized) = {
            let state = self.state.lock().unwrap();
            (state.rpc.clone(), state.pid, state.current.state == LspState::Ready)
        };
        let mut exited = self.exited.subscribe();
        if let Some(rpc) = &rpc {
            if initialized
                && !*exited.borrow()
                && rpc.request("shutdown", Value::Null, Some(SHUTDOWN_RPC)).await.is_ok()
            {
                rpc.notify("exit", Value::Null);
            }
            rpc.dispose("language server shut down".into());
        }
        if let Some(pid) = pid.filter(|_| !*exited.borrow()) {
            signal(pid, false);
            if tokio::time::timeout(KILL_AFTER, exited.wait_for(|exited| *exited)).await.is_err() {
                signal(pid, true);
            }
        }
        if let Err(error) = exited.wait_for(|exited| *exited).await {
            crate::status!("[diffle] language server exit channel closed: {error}");
        }
    }

    pub(super) async fn launch(self: &Arc<Self>, opts: ProcessOptions) -> std::result::Result<(), ()> {
        if self.state.lock().unwrap().closing {
            self.started.send_replace(true);
            self.exited.send_replace(true);
            return Err(());
        }
        let spawned = match command(&opts.command) {
            Ok(mut cmd) => {
                cmd.current_dir(&opts.root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
                spawn_settled(&mut cmd).await.map_err(|error| error.to_string())
            }
            Err(error) => Err(error),
        };
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                self.started.send_replace(true);
                self.exited.send_replace(true);
                return self.fail(format!("cannot start \"{}\": {error}", self.command));
            }
        };
        let (stdin, stdout, stderr) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(stdin), Some(stdout), Some(stderr)) = (stdin, stdout, stderr) else {
            if let Some(pid) = child.id() {
                signal(pid, true);
            }
            let reaped = child.wait().await;
            self.started.send_replace(true);
            self.exited.send_replace(true);
            return self.fail(format!("cannot start \"{}\": no stdio (cleanup: {reaped:?})", self.command));
        };
        let rpc = Connection::new(stdout, stdin);
        {
            let mut state = self.state.lock().unwrap();
            state.rpc = Some(rpc.clone());
            state.pid = child.id();
        }
        tokio::spawn(child::watch(Arc::downgrade(self), stderr, child));
        // close waits for this point before selecting a PID, including when spawn had to retry.
        self.started.send_replace(true);
        if self.state.lock().unwrap().closing {
            return Err(());
        }
        let mut ready = self.ready.subscribe();
        tokio::select! {
            biased;
            _ = ready.wait_for(|ready| *ready == Some(false)) => Err(()),
            result = self.initialize(opts, rpc) => result,
        }
    }

    async fn initialize(self: &Arc<Self>, opts: ProcessOptions, rpc: Connection) -> std::result::Result<(), ()> {
        self.listen(&rpc);
        let root_uri = url::Url::from_file_path(&opts.root).map(String::from).unwrap_or_default();
        let mut params = json!({
            "processId": std::process::id(),
            "clientInfo": { "name": "diffle" },
            "rootUri": root_uri,
            "rootPath": opts.root.to_string_lossy(),
            "workspaceFolders": [{ "uri": root_uri, "name": "workspace" }],
            "capabilities": client_capabilities(),
        });
        if let Some(options) = opts.initialization_options {
            params["initializationOptions"] = options;
        }
        let init = match rpc.request("initialize", params, Some(INIT_TIMEOUT)).await {
            Ok(init) => init,
            Err(error) => {
                if matches!(error, RequestError::Closed(_)) {
                    let mut exited = self.exited.subscribe();
                    let _ = tokio::time::timeout(Duration::from_millis(500), exited.wait_for(|exited| *exited)).await;
                }
                return self.fail(format!("initialize failed: {error}"));
            }
        };
        let capabilities = init.get("capabilities").cloned().unwrap_or_default();
        let advertised: Vec<LspFeature> = all_features()
            .into_iter()
            .filter(|feature| capabilities.get(provider_of(*feature)).is_some_and(truthy))
            .collect();
        self.state.lock().unwrap().advertised.extend(advertised);
        rpc.notify("initialized", json!({}));
        let settings = self.state.lock().unwrap().settings.clone();
        if let Some(settings) = settings {
            let settings = settings.await;
            if !self.state.lock().unwrap().closing {
                rpc.notify("workspace/didChangeConfiguration", json!({ "settings": settings }));
            }
        }
        let status = {
            let mut state = self.state.lock().unwrap();
            if state.closing || state.current.state != LspState::Starting {
                return Err(());
            }
            state.current.state = LspState::Ready;
            state.current.clone()
        };
        (self.on_status)(&status);
        Ok(())
    }
}
