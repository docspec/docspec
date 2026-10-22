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

    /// Largest request body accepted, in bytes. 0 means unlimited.
    #[arg(long, default_value_t = 0, value_name = "BYTES")]
    pub max_body_size: u64,

    /// Longest a single conversion may run, in seconds. 0 means unlimited.
    #[arg(long, default_value_t = 0, value_name = "SECONDS")]
    pub conversion_timeout: u64,
}

impl Args {
    /// Convert CLI arguments into a [`crate::server::ServerConfig`].
    #[inline]
    #[must_use]
    pub fn into_config(self) -> crate::server::ServerConfig {
        crate::server::ServerConfig::new(self.host, self.port).with_limits(
            crate::server::Limits::from_flags(self.max_body_size, self.conversion_timeout),
        )
    }

    /// Create a new [`Args`] with the given host and port and no limits.
    #[inline]
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            max_body_size: 0,
            conversion_timeout: 0,
        }
    }

    /// Sets the body size and conversion time limits, `0` meaning unlimited.
    #[inline]
    pub fn with_limits(mut self, max_body_size: u64, conversion_timeout: u64) -> Self {
        self.max_body_size = max_body_size;
        self.conversion_timeout = conversion_timeout;
        self
    }
}
