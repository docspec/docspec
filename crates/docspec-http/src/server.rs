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

/// Opt-in resource limits for `POST /conversion`.
///
/// Both are off by default; `docspec http --max-body-size` and
/// `--conversion-timeout` turn them on. A request over the body limit is
/// answered `413 Content Too Large` before the body is buffered when
/// `Content-Length` says so, and as soon as the limit is crossed otherwise.
/// A conversion past the timeout is abandoned at the next event and answered
/// `503 Service Unavailable` with `Retry-After`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    /// Largest request body accepted, in bytes. `None` means unlimited.
    pub max_body_bytes: Option<u64>,
    /// Longest a single conversion may run. `None` means unlimited.
    pub conversion_timeout: Option<Duration>,
}

impl Limits {
    /// Builds limits from command-line values where `0` means unlimited.
    #[inline]
    #[must_use]
    pub fn from_flags(max_body_bytes: u64, conversion_timeout_secs: u64) -> Self {
        Self {
            max_body_bytes: (max_body_bytes > 0).then_some(max_body_bytes),
            conversion_timeout: (conversion_timeout_secs > 0)
                .then(|| Duration::from_secs(conversion_timeout_secs)),
        }
    }
}

/// Configuration for the HTTP server.
pub struct ServerConfig {
    /// Network address to bind. Accepts IPv4 literals (`127.0.0.1`), IPv6
    /// literals (`::1`), and hostnames (`localhost`). Default: `127.0.0.1`.
    pub host: String,
    /// Resource limits for the conversion endpoint; unlimited by default.
    pub limits: Limits,
    /// Port to listen on. Use `0` for OS-assigned (for testing).
    pub port: u16,
}

impl ServerConfig {
    /// Create a new server configuration with no resource limits.
    #[inline]
    #[must_use]
    pub fn new<Host>(host: Host, port: u16) -> Self
    where
        Host: Into<String>,
    {
        Self {
            host: host.into(),
            limits: Limits::default(),
            port,
        }
    }

    /// Sets the resource limits for the conversion endpoint.
    #[inline]
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
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

    let server_result = axum::serve(
        listener,
        crate::router::router_with_metrics_and_limits(handle, config.limits),
    )
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
#[inline]
async fn init_posthog() {
    let client = crate::telemetry::init_posthog_client_from_env().await;
    crate::telemetry::install_posthog_client(client);
}

/// Resolves when SIGINT (Ctrl+C) or SIGTERM is received.
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
mod limits_tests {
    use super::*;

    #[test]
    fn zero_flags_mean_unlimited() {
        assert_eq!(Limits::from_flags(0, 0), Limits::default());
        assert_eq!(
            Limits::from_flags(1024, 30),
            Limits {
                max_body_bytes: Some(1024),
                conversion_timeout: Some(Duration::from_secs(30)),
            }
        );
    }

    #[test]
    fn config_carries_limits() {
        let config = ServerConfig::new("127.0.0.1", 0);
        assert_eq!(config.limits, Limits::default());
        let limited = config.with_limits(Limits::from_flags(1, 1));
        assert_eq!(limited.limits.max_body_bytes, Some(1));
        assert_eq!(
            limited.limits.conversion_timeout,
            Some(Duration::from_secs(1))
        );
    }
}
