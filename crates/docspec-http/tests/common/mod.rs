//! Shared test utilities for docspec-http integration tests.
//!
//! Tests that exercise Prometheus metrics use [`with_test_recorder`] to
//! scope a fresh, isolated recorder for the duration of a closure.
//!
//! Importantly: `metrics::with_local_recorder` is THREAD-LOCAL and does
//! NOT propagate into `tokio::task::spawn_blocking`. Tests that exercise
//! blocking work must record metrics in the async context (this matches
//! the production pattern established in the conversion handler).
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used,
    dead_code
)]

use metrics_exporter_prometheus::PrometheusHandle;

/// Runs `body` inside a thread-local Prometheus recorder; returns
/// `(handle, body_result)`. Callers use `handle.render()` to inspect the
/// exposition-format scrape body.
///
/// # Panics
///
/// Panics if the test recorder cannot be built (should never happen in tests).
pub fn with_test_recorder<R, F>(body: F) -> (PrometheusHandle, R)
where
    F: FnOnce() -> R,
{
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("test recorder builds");
    let result = metrics::with_local_recorder(&recorder, body);
    (handle, result)
}
