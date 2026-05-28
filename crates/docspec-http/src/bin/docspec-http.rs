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
#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();
    docspec_http::tracing_init::init();
    let config = ServerConfig::new(args.host, args.port);
    match serve(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = %error, "server error");
            ExitCode::FAILURE
        }
    }
}
