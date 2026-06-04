//! `docspec-http` binary — HTTP API server for `DocSpec` document conversion.

use std::process::ExitCode;

use clap::Parser;
use docspec_http::{serve, server::ServerConfig};

/// `DocSpec` HTTP API server — converts markdown to `BlockNote` JSON.
#[must_use]
#[derive(Debug, Parser)]
#[command(version, about = "DocSpec HTTP API server (markdown → BlockNote JSON)")]
struct Args {
    /// Address to bind the server to.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on. Use 0 for OS-assigned.
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

/// Entry point for the `docspec-http` server binary.
///
/// Parses CLI arguments, initializes tracing, and starts the HTTP server.
/// Returns [`ExitCode::FAILURE`] on bind or serve errors.
fn main() -> ExitCode {
    let args = Args::parse();
    let _telemetry = docspec_http::telemetry::init();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(error) => {
            eprintln!("failed to build tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    runtime.block_on(async move {
        docspec_http::tracing_init::init();
        let body_limit: usize = std::env::var("DOCSPEC_HTTP_MAX_BODY_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(docspec_http::router::DEFAULT_BODY_LIMIT_BYTES);
        let mut config = ServerConfig::new(args.host, args.port);
        config.body_limit_bytes = body_limit;
        match serve(config).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!(error = %error, "docspec-http server error");
                ExitCode::FAILURE
            }
        }
    })
}
