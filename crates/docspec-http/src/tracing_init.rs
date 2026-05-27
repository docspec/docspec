//! Tracing subscriber initialization.
//!
//! Call [`init`] once at process start, before any `tracing::*!` macros.
//! Repeated calls succeed (no-op) once a global subscriber is already
//! installed, which makes the function safe to call from multiple tests.
//! Any other [`tracing_subscriber`] initialization error is propagated to
//! the caller. If the provided `level` is invalid and `RUST_LOG` is not
//! set or is also invalid, returns an error without installing a subscriber.

/// Log output format for the HTTP server.
///
/// This is a plain enum with no `clap` dependency. The CLI crate defines
/// a separate `LogFormatArg` enum with `clap::ValueEnum` and converts via `From`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    /// Machine-readable JSON logs (for production/log aggregation).
    Json,
    /// Human-readable pretty-printed logs (default for development).
    Pretty,
}

/// Initializes the global tracing subscriber.
///
/// `RUST_LOG` takes precedence when it is set to a valid filter directive. Otherwise,
/// `level` is used as the fallback. Repeated calls after a subscriber has already
/// been installed succeed as a no-op.
///
/// # Errors
///
/// Returns an error if neither `RUST_LOG` nor `level` contains a valid tracing filter directive.
///
/// # Examples
///
/// ```no_run
/// use docspec_http::tracing_init::{init, LogFormat};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// init("info", LogFormat::Pretty)?;
/// # Ok(())
/// # }
/// ```
#[inline]
pub fn init(level: &str, format: LogFormat) -> Result<(), Box<dyn core::error::Error>> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new(level))?;

    let result = match format {
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init(),
        LogFormat::Pretty => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .pretty()
            .try_init(),
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) if err.to_string().contains("global default") => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    // Reason: Test code may use unwrap for assertion clarity.

    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_rust_log<T>(rust_log_value: Option<&str>, test: impl FnOnce() -> T) -> T {
        let guard = ENV_LOCK.lock().unwrap();
        let original = std::env::var_os("RUST_LOG");
        match rust_log_value {
            Some(env_value) => std::env::set_var("RUST_LOG", env_value),
            None => std::env::remove_var("RUST_LOG"),
        }
        let result = test();
        match original {
            Some(original_value) => std::env::set_var("RUST_LOG", original_value),
            None => std::env::remove_var("RUST_LOG"),
        }
        drop(guard);
        result
    }

    #[test]
    fn idempotent() {
        // First call
        let r1 = init("info", LogFormat::Pretty);
        assert!(r1.is_ok(), "first init failed: {r1:?}");
        // Second call — must not panic or return Err
        let r2 = init("info", LogFormat::Pretty);
        assert!(r2.is_ok(), "second init failed: {r2:?}");
    }

    #[test]
    fn json_format_initializes() {
        let r = init("debug", LogFormat::Json);
        assert!(r.is_ok(), "json format init failed: {r:?}");
    }

    #[test]
    fn invalid_level_returns_error() {
        with_rust_log(None, || {
            let r = init("docspec=notalevel", LogFormat::Pretty);
            assert!(r.is_err(), "invalid fallback level should error");
        });
    }

    #[test]
    fn rust_log_precedence_allows_invalid_level() {
        with_rust_log(Some("info"), || {
            let r = init("docspec=notalevel", LogFormat::Pretty);
            assert!(r.is_ok(), "valid RUST_LOG should take precedence");
        });
    }

    #[test]
    fn falls_back_to_level_when_rust_log_absent() {
        with_rust_log(None, || {
            let r = init("debug", LogFormat::Pretty);
            assert!(r.is_ok(), "valid fallback level should initialize");
        });
    }

    #[test]
    fn falls_back_to_level_when_rust_log_invalid() {
        with_rust_log(Some("docspec=notalevel"), || {
            let r = init("warn", LogFormat::Pretty);
            assert!(r.is_ok(), "invalid RUST_LOG should fall back to level");
        });
    }

    #[test]
    fn restores_existing_rust_log_after_scoped_test() {
        let guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("RUST_LOG", "error");
        drop(guard);

        with_rust_log(None, || {
            assert!(std::env::var_os("RUST_LOG").is_none());
        });

        assert_eq!(std::env::var("RUST_LOG").unwrap(), "error");
        std::env::remove_var("RUST_LOG");
    }
}
