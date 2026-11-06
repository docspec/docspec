//! `docspec http` subcommand: run the HTTP API server.
//!
//! Thin wrapper around [`docspec_http::run_server`]. The caller (`main.rs`) is
//! responsible for initializing telemetry BEFORE invoking this function and holding the
//! returned `TelemetryGuard` for the rest of the process lifetime.

use crate::args::HttpArgs;
use crate::error::{CliError, Result};
use docspec_http::{run_server, ServerConfig};

/// Execute the `http` subcommand. Delegates to [`docspec_http::run_server`].
///
/// # Lifetime requirement
///
/// The caller (`main.rs`) MUST hold the `TelemetryGuard` returned by
/// `docspec_http::init_telemetry()` until this function returns.
pub fn run(args: HttpArgs) -> Result<()> {
    run_server(ServerConfig::new(args.host, args.port)).map_err(CliError::Http)
}
