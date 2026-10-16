//! Background workers: long-lived tasks that turn the outside world into signals.
//!
//! - [`fs_watcher`]: file-system events, debounced into one "dirty" signal.
//! - [`http_server`]: the listener that serves the API, the page and the live-update socket.
//! - [`browser_opener`]: asks the desktop to show a page.
//! - [`interrupt`]: Ctrl+C and the termination signal.
//! - [`session_watchers`]: offers the file-system watcher to the session.

pub mod browser_opener;
pub mod fs_watcher;
pub mod http_server;
pub mod interrupt;
pub mod session_watchers;
