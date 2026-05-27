//! HTTP server implementation.

use core::net::SocketAddr;

use tokio::net::TcpListener;

/// Configuration for the HTTP server.
pub struct ServerConfig {
    /// Network address to bind (default: `127.0.0.1`).
    pub host: String,
    /// Port to listen on. Use `0` for OS-assigned (for testing).
    pub port: u16,
}

impl ServerConfig {
    /// Create a new server configuration.
    #[inline]
    #[must_use]
    pub fn new<Host>(host: Host, port: u16) -> Self
    where
        Host: Into<String>,
    {
        Self {
            host: host.into(),
            port,
        }
    }
}

/// Bind and start the HTTP server, shutting down gracefully on SIGINT/SIGTERM.
///
/// Logs the actual bound address using [`TcpListener::local_addr`] rather than
/// the configured port, so that port `0` (OS-assigned) shows the real port.
///
/// # Errors
///
/// Returns [`Err`] if the address cannot be parsed, if the port is already in
/// use, or if listening fails.
#[inline]
pub async fn serve(config: ServerConfig) -> std::io::Result<()> {
    let ServerConfig { host, port } = config;
    let bind_target = format!("{host}:{port}");
    let addr: SocketAddr = bind_target.parse().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid bind address `{bind_target}`: {error}"),
        )
    })?;

    let listener = TcpListener::bind(addr).await?;
    let bound_addr = listener.local_addr()?;
    tracing::info!(addr = %bound_addr, "docspec-http listening");

    axum::serve(listener, crate::router::router())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Resolves when SIGINT (Ctrl+C) or SIGTERM is received.
// Reason: graceful shutdown is intentionally factored out to match Axum's
// documented server lifecycle pattern and keep `serve` focused on binding.
#[allow(clippy::single_call_fn)]
#[inline]
async fn shutdown_signal() {
    use core::future;

    use tokio::signal;

    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl+C handler");
            future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler");
                future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("shutdown signal received, draining");
        },
        () = terminate => {
            tracing::info!("shutdown signal received, draining");
        },
    }
}

#[cfg(test)]
mod tests {
    // Reason: server tests use panic for impossible harness failure branches.
    #![allow(clippy::panic)]

    use core::time::Duration;
    use std::io::ErrorKind;

    use tokio::net::TcpListener;

    use super::{serve, ServerConfig};

    #[tokio::test]
    async fn actual_port_logged() {
        let result = TcpListener::bind("127.0.0.1:0").await;
        let listener = match result {
            Ok(listener) => listener,
            Err(error) => panic!("port 0 bind should succeed: {error}"),
        };

        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(error) => panic!("local_addr should succeed: {error}"),
        };

        assert_ne!(addr.port(), 0);
    }

    #[tokio::test]
    async fn bind_succeeds_on_port_zero() {
        let handle = tokio::spawn(serve(ServerConfig::new("127.0.0.1", 0)));

        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.abort();

        match handle.await {
            Err(error) if error.is_cancelled() => {}
            Ok(Ok(())) => {}
            Ok(Err(error)) => panic!("server bind failed: {error}"),
            Err(error) => panic!("server task failed unexpectedly: {error}"),
        }
    }

    #[tokio::test]
    async fn invalid_host_returns_error() {
        let result = serve(ServerConfig::new("not-a-valid-host", 9999)).await;

        match result {
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::InvalidInput);
                assert!(error.to_string().contains("not-a-valid-host"));
            }
            Ok(()) => panic!("invalid host should return an error"),
        }
    }
}
