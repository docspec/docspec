#![forbid(unsafe_code)]
//! HTTP API server for `DocSpec` document conversion.

extern crate alloc;

pub mod cache;
pub mod cli;
pub mod error;
pub mod format;
pub mod handlers;
pub mod metrics;
pub mod mime_parser;
pub mod router;
pub mod server;
pub mod telemetry;
pub mod tracing_init;

pub use cli::Args;
pub use server::{serve, ServerError};

/// Initialize Sentry (if DSN env var set) and return a guard that flushes Sentry on drop.
/// MUST be called BEFORE building the tokio runtime; `main()` MUST hold the returned guard for the entire process lifetime.
#[must_use]
#[inline]
pub fn init_telemetry() -> telemetry::TelemetryGuard {
    telemetry::init()
}

/// Initialize the global tracing subscriber. Called internally by [`run_server`].
/// Panics if called twice (global subscriber is a singleton).
#[inline]
pub fn init_tracing() {
    tracing_init::init();
}

/// Synchronous server entrypoint. Builds a multi-threaded tokio runtime, installs the
/// global tracing subscriber, and runs the HTTP server until SIGINT/SIGTERM.
///
/// # Lifetime requirement
///
/// The caller MUST call [`init_telemetry`] BEFORE this function and hold the returned
/// `TelemetryGuard` for the entire process lifetime.
///
/// # Errors
///
/// Returns [`ServerError`] if runtime construction fails or the HTTP server fails.
#[inline]
pub fn run_server(args: cli::Args) -> Result<(), ServerError> {
    let config = args.into_config();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(ServerError::RuntimeBuild)?;

    runtime.block_on(async {
        init_tracing();
        serve(config).await
    })
}
