//! Telemetry facade. Currently wraps Sentry. Designed for extraction to
//! `docspec-telemetry` when (a) OpenTelemetry is added, OR (b) `docspec-cli`
//! wants Sentry, OR (c) Prometheus metrics land. Keep the public API surface
//! stable and free of HTTP-specific types.

pub mod sentry;

/// Keeps the telemetry client alive until shutdown so buffered events can flush.
pub struct TelemetryGuard {
    inner: Option<::sentry::ClientInitGuard>,
}

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
    let guard = configured_dsn()
        .and_then(|data_source_name| crate::telemetry::sentry::init_sentry(&data_source_name));
    TelemetryGuard { inner: guard }
}

/// Returns a Sentry tracing layer when telemetry is initialized.
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
#[must_use]
#[inline]
pub fn tower_new_layer(
) -> Option<::sentry::integrations::tower::NewSentryLayer<axum::http::Request<axum::body::Body>>> {
    ::sentry::Hub::current()
        .client()
        .map(|_| crate::telemetry::sentry::tower_new_layer())
}

/// Returns a tower HTTP layer that enriches Sentry events when telemetry is initialized.
#[must_use]
#[inline]
pub fn tower_http_layer() -> Option<::sentry::integrations::tower::SentryHttpLayer> {
    ::sentry::Hub::current()
        .client()
        .map(|_| crate::telemetry::sentry::tower_http_layer())
}

fn configured_dsn() -> Option<String> {
    for name in ["DOCSPEC_SENTRY_DSN", "SENTRY_DSN"] {
        match std::env::var(name) {
            Ok(value) if !value.is_empty() => return Some(value),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_MUTEX.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    #[test]
    fn telemetry_init_returns_noop_guard_when_dsn_absent() {
        let _env_guard = lock_env();
        std::env::remove_var("DOCSPEC_SENTRY_DSN");
        std::env::remove_var("SENTRY_DSN");

        let _telemetry = crate::telemetry::init();

        assert!(crate::telemetry::tracing_layer::<tracing_subscriber::Registry>().is_none());
        assert!(crate::telemetry::tower_new_layer().is_none());
    }

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

    #[test]
    fn resolve_dsn_treats_empty_string_as_absent() {
        let _env_guard = lock_env();
        std::env::set_var("DOCSPEC_SENTRY_DSN", "");
        std::env::remove_var("SENTRY_DSN");

        assert_eq!(super::configured_dsn(), None);
    }
}
