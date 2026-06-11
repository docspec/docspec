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
    clippy::arbitrary_source_item_ordering,
    clippy::expect_used,
    clippy::impl_trait_in_params,
    clippy::shadow_unrelated,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    dead_code
)]

use axum::{body::Body, http::Request, Router};
use metrics_exporter_prometheus::PrometheusHandle;

// Reason: this module is included by every integration test binary in the crate.
// Not every binary uses the re-export, so the unused-import warning is unavoidable
// per-binary without splitting the module per test file.
#[allow(unused_imports)]
pub use docspec_test_utils::synth_docx;

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

/// Minimal `_rels/.rels` XML for a DOCX archive pointing at `word/document.xml`.
pub const SIMPLE_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

/// Builds a valid DOCX conversion request.
///
/// # Panics
///
/// Panics if the request builder rejects the supplied body.
pub fn docx_request(body: impl Into<Body>) -> Request<Body> {
    request(
        "POST",
        "/conversion",
        &[(
            "content-type",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )],
        body,
    )
}
