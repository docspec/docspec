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

use axum::{body::Body, http::Request, Router};
use metrics_exporter_prometheus::PrometheusHandle;

/// Builds a single-threaded Tokio runtime for synchronous integration tests.
///
/// # Panics
///
/// Panics if the runtime cannot be built.
pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime builds")
}

/// Builds a router with a throw-away handle for the `/metrics` route.
///
/// Metric values are captured by the thread-local recorder active at request
/// time — the dummy handle only satisfies `router_with_metrics`'s signature.
///
/// # Panics
///
/// Panics if the test recorder cannot be built.
pub fn router() -> Router {
    let (_, dummy_handle) = docspec_http::metrics::build_recorder().expect("dummy recorder");
    docspec_http::router::router_with_metrics(dummy_handle)
}

/// Builds an HTTP request for integration tests.
///
/// # Panics
///
/// Panics if the request builder rejects the supplied method, URI, headers, or body.
pub fn request<T>(method: &str, uri: &str, headers: &[(&str, &str)], body: T) -> Request<Body>
where
    T: Into<Body>,
{
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(body.into()).expect("request builds")
}

/// Builds an empty-body HTTP request.
pub fn empty_request(method: &str, uri: &str) -> Request<Body> {
    request(method, uri, &[], Body::empty())
}

/// Builds a valid markdown conversion request.
pub fn markdown_request<T>(body: T) -> Request<Body>
where
    T: Into<Body>,
{
    request(
        "POST",
        "/conversion",
        &[("content-type", "text/markdown")],
        body,
    )
}

/// Builds a valid markdown conversion request with an explicit `Accept` header.
pub fn accepted_markdown_request<T>(body: T) -> Request<Body>
where
    T: Into<Body>,
{
    request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/vnd.docspec.blocknote+json"),
        ],
        body,
    )
}

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
