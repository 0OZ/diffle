//! The composition root: a run, step by step. It names each part and holds no logic of its
//! own — read [`run`] and you have read what `diffle` does.
//!
//! 1. [`cli`] reads the command line; [`config`] turns it into domain types.
//! 2. The stored preferences are opened.
//! 3. Either [`settings_command`] edits them, or [`serving`] runs one review: resolve it,
//!    listen, open the browser, wait for its end, hand the open comments over on stdout.
//! 4. [`app_error`] says how a failure is spelled and which exit code it earns.

mod app_error;
mod cli;
mod config;
pub mod run;
mod serving;
mod settings_command;
