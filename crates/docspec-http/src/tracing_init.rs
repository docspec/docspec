//! Tracing subscriber initialization.

use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _};

/// Installs a hardcoded tracing subscriber writing to stderr at INFO level with pretty format.
///
/// # Panics
///
/// Panics if a global subscriber has already been installed. In tests, use [`try_init`] instead.
#[inline]
pub fn init() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .pretty()
                .with_filter(tracing::level_filters::LevelFilter::INFO),
        )
        .with(crate::telemetry::tracing_layer())
        .init();
}

/// Installs the tracing subscriber, returning an error if one is already set.
///
/// Use this in tests to avoid panics from double-initialization.
///
/// # Errors
///
/// Returns [`Err`] if a global subscriber has already been installed.
#[inline]
#[allow(clippy::std_instead_of_core)]
// Reason: `std::error::Error` is not available in `core`; the `tracing_subscriber`
// crate's `try_init` error type requires `std::error::Error + Send + Sync`.
pub fn try_init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .pretty()
                .with_filter(tracing::level_filters::LevelFilter::INFO),
        )
        .with(crate::telemetry::tracing_layer())
        .try_init()
        .map_err(Into::into)
}
