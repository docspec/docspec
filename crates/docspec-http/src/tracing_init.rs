//! Tracing subscriber initialization.

use tracing_subscriber::FmtSubscriber;

/// Installs a hardcoded tracing subscriber writing to stderr at INFO level with pretty format.
///
/// # Panics
///
/// Panics if a global subscriber has already been installed. In tests, use [`try_init`] instead.
#[inline]
pub fn init() {
    FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .pretty()
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
    FmtSubscriber::builder()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .pretty()
        .try_init()
}
