//! Server lifecycle: bind, serve, graceful shutdown.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};

use crate::router;

/// Starts the HTTP server and blocks until a shutdown signal is received.
///
/// Binds to `host:port` and serves the `DocSpec` API. Shuts down gracefully on
/// `SIGINT` (Ctrl-C) or `SIGTERM` (Unix only), draining in-flight requests
/// without a timeout.
///
/// # Errors
///
/// Returns an error if the address cannot be parsed or the port is already in use.
///
/// # Examples
///
/// ```no_run
/// # async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
/// docspec_http::serve("127.0.0.1", 3000).await?;
/// # Ok(())
/// # }
/// ```
#[inline]
pub async fn serve(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    serve_until(host, port, shutdown_signal()).await
}

// Reason: Testable server lifecycle helper is intentionally called only through `serve` in production.
#[allow(clippy::single_call_fn)]
async fn serve_until<F>(
    host: &str,
    port: u16,
    shutdown: F,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: Future<Output = ()> + Send + 'static,
{
    let ip: IpAddr = host.parse()?;
    let addr = SocketAddr::from((ip, port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound_addr = listener.local_addr()?;
    tracing::info!(addr = %bound_addr, "docspec-http listening");
    axum::serve(listener, router::router())
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// Waits for a shutdown signal: Ctrl-C on all platforms, SIGTERM on Unix.
// Reason: This lifecycle helper is intentionally called only by `serve`.
#[allow(clippy::single_call_fn)]
async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            tracing::warn!("failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(unix)]
    wait_for_shutdown(ctrl_c, terminate).await;

    #[cfg(not(unix))]
    wait_for_shutdown(ctrl_c).await;
}

#[cfg(unix)]
// Reason: Extracted to keep shutdown race selection directly testable.
#[allow(clippy::single_call_fn)]
async fn wait_for_shutdown<C, T>(ctrl_c: C, terminate: T)
where
    C: Future<Output = ()>,
    T: Future<Output = ()>,
{
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("graceful shutdown initiated");
}

#[cfg(not(unix))]
async fn wait_for_shutdown<C>(ctrl_c: C)
where
    C: Future<Output = ()>,
{
    ctrl_c.await;
    tracing::info!("graceful shutdown initiated");
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::single_call_fn, clippy::unwrap_used)]
    // Reason: Test code uses panic/unwrap for assertion clarity; helpers are called once per test.

    use std::time::Duration;

    use tokio::net::TcpListener;

    #[tokio::test]
    async fn bind_in_use_errors() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let result = crate::serve("127.0.0.1", port).await;

        assert!(result.is_err(), "expected bind to fail on occupied port");
        drop(listener);
    }

    #[tokio::test]
    async fn start_and_shutdown() {
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            super::serve_until("127.0.0.1", 0, async {}),
        )
        .await;

        match result {
            Ok(Ok(())) => {}
            Err(elapsed) => panic!("server did not shut down within timeout: {elapsed}"),
            Ok(Err(e)) => panic!("server failed to start: {e}"),
        }
    }

    #[tokio::test]
    async fn ipv6_bind_supported() {
        if TcpListener::bind("[::1]:0").await.is_err() {
            return;
        }

        let result = tokio::time::timeout(
            Duration::from_millis(200),
            super::serve_until("::1", 0, async {}),
        )
        .await;

        match result {
            Ok(Ok(())) => {}
            Err(elapsed) => panic!("IPv6 server did not shut down within timeout: {elapsed}"),
            Ok(Err(e)) => panic!("IPv6 server failed to start: {e}"),
        }
    }

    #[tokio::test]
    async fn invalid_host_returns_error() {
        let result = super::serve_until("not an ip address", 0, async {}).await;

        assert!(result.is_err(), "expected host parsing to fail");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_helper_returns_on_ctrl_c() {
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            super::wait_for_shutdown(async {}, std::future::pending::<()>()),
        )
        .await;

        assert!(result.is_ok(), "shutdown helper did not return on ctrl-c");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_helper_returns_on_sigterm() {
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            super::wait_for_shutdown(std::future::pending::<()>(), async {}),
        )
        .await;

        assert!(result.is_ok(), "shutdown helper did not return on SIGTERM");
    }
}
