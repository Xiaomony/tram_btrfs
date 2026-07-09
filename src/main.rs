mod cli;
mod core;
mod globals;
mod tui;

use cli::Cli;
use color_eyre::{config::HookBuilder, eyre::Context};
use tracing_error::ErrorLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::core::error::CResult;

fn main() -> CResult<()> {
    #[cfg(not(debug_assertions))]
    let debug_mode = std::env::var("DEBUG").is_ok_and(|v| v == "1");
    #[cfg(debug_assertions)]
    let debug_mode = true;

    tracing_subscriber::registry()
        .with(ErrorLayer::default())
        .init();

    HookBuilder::default()
        .display_env_section(false)
        .capture_span_trace_by_default(debug_mode)
        .display_location_section(debug_mode)
        .install()?;
    let result = Cli::process_args();
    if debug_mode {
        result
    } else {
        result.wrap_err(">>> If you intend to report this as a bug, please reproduce the bug by 'DEBUG=1 sudo tram' and collect output. <<<").wrap_err("=== Skip this error chain and read sections below first! ===")
    }
}
