//! HTTP server implementation.

use core::time::Duration;

use metrics_exporter_prometheus::BuildError;
use tokio::net::TcpListener;

/// Errors that can occur while starting or running the HTTP server.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ServerError {
    /// A TCP bind or Axum serve I/O failure.
    #[error(transparent)]
    Listen(#[from] std::io::Error),

    /// Failed to install the Prometheus metrics recorder.
    #[error("failed to initialize Prometheus metrics recorder: {0}")]
    MetricsInit(#[source] BuildError),

    /// Failed to build the tokio runtime.
    #[error("failed to build tokio runtime: {0}")]
    RuntimeBuild(#[source] std::io::Error),
}

/// Configuration for the HTTP server.
pub struct ServerConfig {
    /// Network address to bind. Accepts IPv4 literals (`127.0.0.1`), IPv6
    /// literals (`::1`), and hostnames (`localhost`). Default: `127.0.0.1`.
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

/// Bind a TCP listener for the given configuration without starting the
/// Axum server.
///
/// The `host` field accepts IPv4 literals, IPv6 literals (no brackets needed),
/// and hostnames (resolved via DNS).
///
/// Useful for tests that need to assert binding behavior deterministically
/// without spawning the full server task.
///
/// # Errors
///
/// Returns [`Err`] if the host cannot be resolved, the port is already in
/// use, or listening fails.
#[inline]
pub async fn bind(config: &ServerConfig) -> std::io::Result<TcpListener> {
    TcpListener::bind((config.host.as_str(), config.port)).await
}

/// Bind and start the HTTP server, shutting down gracefully on SIGINT/SIGTERM.
///
/// Delegates binding to [`bind`], installs the Prometheus metrics recorder
/// globally, then spawns an upkeep task that is aborted when the server future
/// completes. Logs the actual bound address using [`TcpListener::local_addr`]
/// rather than the configured port, so that port `0` (OS-assigned) shows the
/// real port.
///
/// # Errors
///
/// Returns [`Err`] if the Prometheus recorder cannot be installed, the host
/// cannot be resolved, the port is already in use, or listening fails.
#[inline]
pub async fn serve(config: ServerConfig) -> Result<(), ServerError> {
    let listener = bind(&config).await?;
    let bound_addr = listener.local_addr()?;

    let handle = crate::metrics::install_global().map_err(ServerError::MetricsInit)?;

    #[cfg(feature = "posthog")]
    init_posthog().await;

    let upkeep_task = spawn_upkeep(handle.clone());

    tracing::info!(addr = %bound_addr, "docspec-http listening");

    let server_result = axum::serve(listener, crate::router::router_with_metrics(handle))
        .with_graceful_shutdown(shutdown_signal())
        .await;

    crate::telemetry::shutdown().await;
    #[cfg(feature = "posthog")]
    tracing::info!("posthog flush complete");

    teardown_upkeep(upkeep_task).await;
    server_result?;

    Ok(())
}

/// Spawns the Prometheus upkeep task.
#[allow(clippy::single_call_fn)]
#[inline]
fn spawn_upkeep(
    handle: metrics_exporter_prometheus::PrometheusHandle,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            handle.run_upkeep();
        }
    })
}

/// Aborts the upkeep task and awaits its completion.
#[allow(clippy::single_call_fn)]
#[inline]
async fn teardown_upkeep(task: tokio::task::JoinHandle<()>) {
    task.abort();
    match task.await {
        Err(error) if error.is_cancelled() => {}
        Err(error) => tracing::error!(%error, "metrics upkeep task failed during shutdown"),
        Ok(()) => {}
    }
}

/// Initialises the `PostHog` client from environment variables and installs
/// it — or clears any stale predecessor — in the shared slot so capture
/// sites can reach it.
///
/// Always calls [`crate::telemetry::install_posthog_client`], even when the
/// factory returns `None`, so that a client left in the slot by a previous
/// run in the same process cannot linger into this run.
#[cfg(feature = "posthog")]
#[allow(clippy::single_call_fn)]
#[inline]
async fn init_posthog() {
    let client = crate::telemetry::init_posthog_client_from_env().await;
    crate::telemetry::install_posthog_client(client);
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
