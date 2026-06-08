//! `docspec http` subcommand: run the HTTP API server.
//!
//! Thin wrapper around [`docspec_http::run_server`]. The caller (`main.rs`) is
//! responsible for initializing telemetry BEFORE invoking this function and holding the
//! returned `TelemetryGuard` for the rest of the process lifetime.

use crate::args::HttpArgs;
use crate::error::{CliError, Result};
use docspec_http::{cli::Args as DocspecHttpArgs, run_server};

impl From<HttpArgs> for DocspecHttpArgs {
    #[inline]
    fn from(args: HttpArgs) -> Self {
        Self::new(args.host, args.port)
    }
}

/// Execute the `http` subcommand. Delegates to [`docspec_http::run_server`].
///
/// # Lifetime requirement
///
/// The caller (`main.rs`) MUST hold the `TelemetryGuard` returned by
/// `docspec_http::init_telemetry()` until this function returns.
pub fn run(args: HttpArgs) -> Result<()> {
    let dh_args: DocspecHttpArgs = args.into();
    run_server(dh_args).map_err(CliError::Http)
}
