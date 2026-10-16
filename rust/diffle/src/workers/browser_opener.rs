//! Opens a URL in the desktop's default browser, and does not wait for it.

use std::process::{Command, Stdio};

const OPENER_MACOS: &str = "open";
const OPENER_WINDOWS: &str = "cmd";
/// `start` takes the first quoted argument as a window title, hence the empty one.
const WINDOWS_START: [&str; 3] = ["/c", "start", ""];
const OPENER_OTHER: &str = "xdg-open";

/// Whether the opener could be started. It is not waited on: the browser outlives the review.
#[must_use]
pub fn open_in_browser(url: &str) -> bool {
    let mut command = opener(url);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
}

fn opener(url: &str) -> Command {
    let (program, prefix): (&str, &[&str]) = match std::env::consts::OS {
        "macos" => (OPENER_MACOS, &[]),
        "windows" => (OPENER_WINDOWS, &WINDOWS_START),
        _ => (OPENER_OTHER, &[]),
    };
    let mut command = Command::new(program);
    command.args(prefix).arg(url);
    command
}

#[cfg(test)]
#[path = "browser_opener_tests.rs"]
mod tests;
