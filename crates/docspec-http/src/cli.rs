//! Command-line interface arguments for the `docspec-http` server binary.

use clap::Parser;

/// `DocSpec` HTTP API server — converts markdown to `BlockNote` JSON.
#[must_use]
#[non_exhaustive]
#[derive(Parser, Debug, Clone)]
#[command(version, about = "DocSpec HTTP API server (markdown → BlockNote JSON)")]
pub struct Args {
    /// Address to bind the server to.
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on. Use 0 for OS-assigned.
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

impl Args {
    /// Convert CLI arguments into a [`crate::server::ServerConfig`].
    #[inline]
    #[must_use]
    pub fn into_config(self) -> crate::server::ServerConfig {
        crate::server::ServerConfig::new(self.host, self.port)
    }

    /// Create a new [`Args`] with the given host and port.
    #[inline]
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }
}
