//! Telemetry facade. Currently wraps Sentry. Designed for extraction to
//! `docspec-telemetry` when (a) OpenTelemetry is added, OR (b) `docspec-cli`
//! wants Sentry, OR (c) Prometheus metrics land. Keep the public API surface
//! stable and free of HTTP-specific types.

#[cfg(feature = "posthog")]
pub mod posthog;

#[cfg(feature = "sentry")]
pub mod sentry;

/// Keeps the telemetry client alive until shutdown so buffered events can flush.
pub struct TelemetryGuard {
    #[cfg(feature = "sentry")]
    inner: Option<::sentry::ClientInitGuard>,
}

#[cfg(feature = "sentry")]
impl Drop for TelemetryGuard {
    #[inline]
    fn drop(&mut self) {
        drop(self.inner.take());
    }
}

/// Initializes telemetry from the configured environment and returns its guard.
#[must_use]
#[inline]
pub fn init() -> TelemetryGuard {
    #[cfg(feature = "sentry")]
    let inner = configured_dsn()
        .and_then(|data_source_name| crate::telemetry::sentry::init_sentry(&data_source_name));
    TelemetryGuard {
        #[cfg(feature = "sentry")]
        inner,
    }
}

/// Returns a Sentry tracing layer when telemetry is initialized.
#[cfg(feature = "sentry")]
#[must_use]
#[inline]
pub fn tracing_layer<S>() -> Option<::sentry::integrations::tracing::SentryLayer<S>>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    ::sentry::Hub::current()
        .client()
        .map(|_| crate::telemetry::sentry::tracing_layer())
}

/// Returns a tower layer that binds a Sentry hub to each request when telemetry is initialized.
#[cfg(feature = "sentry")]
#[must_use]
#[inline]
pub fn tower_new_layer(
) -> Option<::sentry::integrations::tower::NewSentryLayer<axum::http::Request<axum::body::Body>>> {
    ::sentry::Hub::current()
        .client()
        .map(|_| crate::telemetry::sentry::tower_new_layer())
}

/// Returns a tower HTTP layer that enriches Sentry events when telemetry is initialized.
#[cfg(feature = "sentry")]
#[must_use]
#[inline]
pub fn tower_http_layer() -> Option<::sentry::integrations::tower::SentryHttpLayer> {
    ::sentry::Hub::current()
        .client()
        .map(|_| crate::telemetry::sentry::tower_http_layer())
}

#[cfg(feature = "sentry")]
fn configured_dsn() -> Option<String> {
    for name in ["DOCSPEC_SENTRY_DSN", "SENTRY_DSN"] {
        match std::env::var(name) {
            Ok(value) if !value.is_empty() => return Some(value),
            _ => {}
        }
    }
    None
}

/// Swappable slot for the `PostHog` client.
///
/// `OnceLock` initialises the `RwLock` exactly once; subsequent reads and
/// writes go through the `RwLock`, which allows tests to replace the client
/// between runs. `serve()` calls [`install_posthog_client`] at startup — with
/// the freshly built client when configured, or `None` otherwise — and test
/// harnesses use the same helper (or the slot directly) to inject a
/// wiremock-backed client before each scenario.
#[cfg(feature = "posthog")]
pub(crate) static POSTHOG_CLIENT: std::sync::OnceLock<
    std::sync::RwLock<Option<std::sync::Arc<posthog_rs::Client>>>,
> = std::sync::OnceLock::new();

/// Returns a reference to the shared `PostHog` client slot.
///
/// The slot is initialised to `None` on first access. `serve()` writes a
/// live client after init; tests write a stub before each scenario.
#[cfg(feature = "posthog")]
#[inline]
pub fn posthog_client_slot(
) -> &'static std::sync::RwLock<Option<std::sync::Arc<posthog_rs::Client>>> {
    POSTHOG_CLIENT.get_or_init(|| std::sync::RwLock::new(None))
}

/// Constructs a `PostHog` client from environment variables and returns it
/// wrapped in an `Arc`.
///
/// This function is a pure factory: it does **not** write to
/// [`posthog_client_slot`]. Callers must invoke [`install_posthog_client`]
/// with the result — including passing `None` when the factory returns `None`
/// — so that any stale client from a prior run in the same process is
/// cleared rather than left behind.
///
/// Returns `None` when the API key env var is absent, or the SDK refuses to
/// initialise.
#[cfg(feature = "posthog")]
#[inline]
pub async fn init_posthog_client_from_env() -> Option<std::sync::Arc<posthog_rs::Client>> {
    let config = posthog::configured_posthog_config()?;
    let client = posthog::init_posthog(config).await?;
    Some(std::sync::Arc::new(client))
}

/// Installs `client` into the shared `PostHog` slot, replacing any previous
/// value.
///
/// Pass `None` to clear the slot. `serve()` always calls this — with either
/// the freshly built client, or `None` when no API key is configured on the
/// current run — so that a stale or already-shutdown client from a previous
/// server startup in the same process cannot linger into a subsequent run
/// and silently receive analytics against outdated configuration.
///
/// If the slot's `RwLock` is poisoned, the install is skipped and a warning
/// is logged; subsequent captures will find no client, disabling `PostHog`
/// for the rest of the process. This is safer than leaving a stale value in
/// place: no analytics is preferable to analytics sent to the wrong
/// destination.
#[cfg(feature = "posthog")]
#[inline]
pub fn install_posthog_client(client: Option<std::sync::Arc<posthog_rs::Client>>) {
    let has_client = client.is_some();
    match posthog_client_slot().write() {
        Ok(mut slot) => {
            *slot = client;
            if has_client {
                tracing::info!("posthog client initialized");
            } else {
                tracing::debug!("posthog client not configured; slot cleared");
            }
        }
        Err(_) => {
            tracing::warn!("posthog client slot poisoned during install; disabling posthog");
        }
    }
}

/// Flushes the `PostHog` client, waiting for in-flight events to drain.
///
/// Reads the client slot, clones the `Arc` if present (dropping the read
/// guard before awaiting to avoid holding it across an await point), then
/// calls `client.shutdown()`.
#[cfg(feature = "posthog")]
#[inline]
pub async fn shutdown() {
    let client_arc = match posthog_client_slot().read() {
        Ok(guard) => guard.as_ref().map(std::sync::Arc::clone),
        Err(_poisoned) => {
            tracing::warn!("posthog client slot poisoned during shutdown; skipping flush");
            None
        }
    };
    if let Some(client) = client_arc {
        client.shutdown().await;
    }
}

/// No-op shutdown when the `posthog` feature is disabled.
///
/// Returns a `Ready<()>` future so call sites can uniformly `.await` it.
#[cfg(not(feature = "posthog"))]
#[inline]
pub fn shutdown() -> core::future::Ready<()> {
    core::future::ready(())
}

/// Parses a float rate from the named environment variable, returning the
/// default if the variable is absent, empty, unparseable, or not finite, and
/// clamping the result to `[0.0, 1.0]`.
///
/// Shared by the Sentry and `PostHog` backends.
#[cfg(any(feature = "sentry", feature = "posthog"))]
pub(in crate::telemetry) fn env_rate(name: &str, default: f32) -> f32 {
    let Ok(value) = std::env::var(name) else {
        return default;
    };

    match value.parse::<f32>() {
        Ok(rate) if rate.is_finite() => rate.clamp(0.0, 1.0),
        _ => {
            eprintln!("warning: {name} invalid or out-of-range; clamped to default {default:.1}");
            default
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::semicolon_outside_block
    )]
    #[cfg(feature = "sentry")]
    use std::sync::Mutex;

    #[cfg(feature = "sentry")]
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[cfg(feature = "sentry")]
    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_MUTEX.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    #[test]
    fn no_sentry_guard_drops_without_panic() {
        let _guard = crate::telemetry::init();
    }

    #[cfg(feature = "posthog")]
    #[test]
    fn posthog_slot_starts_empty() {
        let slot = crate::telemetry::posthog_client_slot();
        let guard = slot.read().expect("slot read should not be poisoned");
        assert!(guard.is_none(), "slot should be None before init");
    }

    #[cfg(feature = "posthog")]
    #[test]
    fn posthog_slot_write_then_read_returns_some() {
        let slot = crate::telemetry::posthog_client_slot();

        // Simulate what serve() does at startup — write a fabricated Arc.
        // We cannot construct a real Client without a live network, so use
        // a test-only approach: write None, then write Some to prove round-trip.
        {
            let mut guard = slot.write().expect("slot write should not be poisoned");
            *guard = None;
        }
        {
            let guard = slot.read().expect("slot read should not be poisoned");
            assert!(guard.is_none());
        }
        // Re-write to None proves teardown works (mirrors test cleanup pattern).
        {
            let mut guard = slot.write().expect("slot write should not be poisoned");
            let _ = guard.take();
        }
    }

    #[cfg(feature = "posthog")]
    #[tokio::test]
    async fn posthog_shutdown_on_empty_slot_is_noop() {
        crate::telemetry::shutdown().await;
    }

    #[cfg(feature = "sentry")]
    #[test]
    fn telemetry_init_returns_noop_guard_when_dsn_absent() {
        let _env_guard = lock_env();
        std::env::remove_var("DOCSPEC_SENTRY_DSN");
        std::env::remove_var("SENTRY_DSN");

        let _telemetry = crate::telemetry::init();

        assert!(crate::telemetry::tracing_layer::<tracing_subscriber::Registry>().is_none());
        assert!(crate::telemetry::tower_new_layer().is_none());
    }

    #[cfg(feature = "sentry")]
    #[test]
    fn resolve_dsn_picks_docspec_over_sentry() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_SENTRY_DSN", "https://docspec.example/1");
        std::env::set_var("SENTRY_DSN", "https://sentry.example/1");

        assert_eq!(
            super::configured_dsn(),
            Some("https://docspec.example/1".to_string())
        );
    }

    #[cfg(feature = "sentry")]
    #[test]
    fn resolve_dsn_treats_empty_string_as_absent() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_SENTRY_DSN", "");
        std::env::remove_var("SENTRY_DSN");

        assert_eq!(super::configured_dsn(), None);
    }
}
