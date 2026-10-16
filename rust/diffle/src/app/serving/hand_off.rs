//! The review's product: its open comments, as the prompt an agent reads.

use std::io::{self, Write};

use crate::domains::comments::prompt::agent_prompt::format_prompt;
use crate::domains::comments::threads::thread_query::{ThreadQuery, ThreadState};
use crate::domains::review::session::review_session::Session;

/// The prompt for every open thread; empty when the review never opened or its comments cannot
/// be read, since this runs while the program shuts down and has nowhere to fail to.
pub async fn open_threads_prompt(session: &Session) -> String {
    let Ok(active) = session.active() else {
        return String::new();
    };
    match active
        .comments()
        .threads(&ThreadQuery::new(ThreadState::Open, None))
        .await
    {
        Ok(threads) => format_prompt(&threads),
        Err(error) => {
            tracing::warn!(%error, "could not read the open comments");
            String::new()
        }
    }
}

/// Writes the prompt to stdout, which belongs to nothing else.
pub async fn hand_off(session: &Session) {
    let prompt = open_threads_prompt(session).await;
    let mut stdout = io::stdout().lock();
    if let Err(error) = stdout
        .write_all(prompt.as_bytes())
        .and_then(|()| stdout.flush())
    {
        tracing::warn!(%error, "could not write the prompt to stdout");
    }
}

#[cfg(test)]
#[path = "hand_off_tests.rs"]
mod tests;
