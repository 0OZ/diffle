//! Status output. It goes to stderr: stdout belongs to the prompt handed to an agent.

use tracing_subscriber::EnvFilter;

const FILTER_VAR: &str = "DIFFLE_LOG";
const DEFAULT_FILTER: &str = "info";

/// Messages carry their own marker (🚀, ✖), so the line has no timestamp, target, or level.
pub fn init() {
    let filter =
        EnvFilter::try_from_env(FILTER_VAR).unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    // A second initialisation (tests) keeps the first subscriber.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .with_level(false)
        .try_init();
}
