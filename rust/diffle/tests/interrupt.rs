//! Real signals, sent to this test process. It runs alone in its binary, and as one test, so
//! no other listener hears them.

#![cfg(unix)]

use std::error::Error;
use std::time::Duration;

use diffle::workers::interrupt::Interrupts;
use rustix::process::{Signal, getpid, kill_process};

const PATIENCE: Duration = Duration::from_secs(10);
/// `ctrl_c` installs its handler when first polled; a signal sent before that is not heard.
const HANDLER_INSTALLED: Duration = Duration::from_millis(200);

#[tokio::test]
async fn each_of_the_two_signals_ends_the_wait() -> Result<(), Box<dyn Error>> {
    let mut interrupts = Interrupts::listen()?;
    kill_process(getpid(), Signal::TERM)?;
    tokio::time::timeout(PATIENCE, interrupts.wait()).await?;

    let waiting = tokio::spawn(async move { interrupts.wait().await });
    tokio::time::sleep(HANDLER_INSTALLED).await;
    kill_process(getpid(), Signal::INT)?;
    tokio::time::timeout(PATIENCE, waiting).await??;
    Ok(())
}
