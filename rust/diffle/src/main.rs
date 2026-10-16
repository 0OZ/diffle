//! The `diffle` executable. Everything it does is spelled out in [`diffle::app`].

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    diffle::app::run::run().await
}
