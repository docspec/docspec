//! Tracing subscriber initialization.

use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _};

/// Installs a hardcoded tracing subscriber writing to stderr at INFO level with pretty format.
///
/// # Panics
///
/// Panics if a global subscriber has already been installed. In tests, use [`try_init`] instead.
#[inline]
pub fn init() {
    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .pretty()
        .with_filter(tracing::level_filters::LevelFilter::INFO);

    #[cfg(feature = "sentry")]
    tracing_subscriber::registry()
        .with(fmt)
        .with(crate::telemetry::tracing_layer())
        .init();

    #[cfg(not(feature = "sentry"))]
    tracing_subscriber::registry().with(fmt).init();
}

/// Installs the tracing subscriber, returning an error if one is already set.
///
/// Use this in tests to avoid panics from double-initialization.
///
/// # Errors
///
/// Returns [`Err`] if a global subscriber has already been installed.
#[inline]
pub fn try_init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .pretty()
        .with_filter(tracing::level_filters::LevelFilter::INFO);

    #[cfg(feature = "sentry")]
    return tracing_subscriber::registry()
        .with(fmt)
        .with(crate::telemetry::tracing_layer())
        .try_init()
        .map_err(Into::into);

    #[cfg(not(feature = "sentry"))]
    tracing_subscriber::registry()
        .with(fmt)
        .try_init()
        .map_err(Into::into)
}
