//! Integration tests for conversion outcome metrics.
//!
//! Each test uses a fresh, isolated Prometheus recorder via
//! [`docspec_http::metrics::build_recorder`] and [`metrics::with_local_recorder`].
//! No global recorder is installed.
//!
//! Tests are synchronous: each test creates its own
//! `tokio::runtime::Runtime` and drives the HTTP request inside
//! [`metrics::with_local_recorder`], so the thread-local recorder is active
//! throughout the full request — including after any `spawn_blocking` join.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use axum::body::Body;
use tower::ServiceExt as _;

/// Asserts that the Prometheus exposition text `rendered` contains a line that
/// is an exact string match of `expected_line`.
fn assert_metric_line(rendered: &str, expected_line: &str) {
    let found = rendered.lines().any(|line| line == expected_line);
    assert!(
        found,
        "Expected metric line not found.\n  expected: {expected_line}\n  rendered:\n{rendered}",
    );
}

// ─── Test 1: success ─────────────────────────────────────────────────────────

/// A successful conversion increments `docspec_conversions_total` with
/// `result="success"` and `error_class="none"`.
#[test]
fn success_increments_with_none_error_class() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello World")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="success",error_class="none"} 1"#,
    );
}

// ─── Test 2: empty body ───────────────────────────────────────────────────────

/// An empty request body increments `docspec_conversions_total` with
/// `result="client_error"` and `error_class="empty_body"`.
#[test]
fn empty_body_records_client_error() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="empty_body"} 1"#,
    );
}

// ─── Test 3: invalid UTF-8 ────────────────────────────────────────────────────

/// A request body with invalid UTF-8 bytes increments `docspec_conversions_total`
/// with `result="client_error"` and `error_class="body_not_utf8"`.
#[test]
fn invalid_utf8_records_client_error() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::accepted_markdown_request(Body::from(b"\xFF\xFE\x00".to_vec()));

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="body_not_utf8"} 1"#,
    );
}

// ─── Test 4: unsupported media type ──────────────────────────────────────────

/// A request with an unsupported `Content-Type` increments
/// `docspec_conversions_total` with `result="client_error"` and
/// `error_class="unsupported_media_type"`.
#[test]
fn unsupported_media_type_records_client_error() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "application/json"),
            ("accept", "application/vnd.docspec.blocknote+json"),
        ],
        Body::from(r#"{"hello": "world"}"#),
    );

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="unsupported_media_type"} 1"#,
    );
}

// ─── Test 5: not acceptable ───────────────────────────────────────────────────

/// A request with a non-matching `Accept` header increments
/// `docspec_conversions_total` with `result="client_error"` and
/// `error_class="not_acceptable"`.
#[test]
fn not_acceptable_records_client_error() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    let request = common::request(
        "POST",
        "/conversion",
        &[
            ("content-type", "text/markdown"),
            ("accept", "application/json"),
        ],
        Body::from("# Hello"),
    );

    metrics::with_local_recorder(&recorder, || rt.block_on(router.oneshot(request)))
        .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversions_total{result="client_error",error_class="not_acceptable"} 1"#,
    );
}

// ─── Test 6: duration on success ─────────────────────────────────────────────

/// A successful conversion records one observation in
/// `docspec_conversion_duration_seconds` with `result="success"`.
#[test]
fn conversion_duration_recorded_on_success() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversion_duration_seconds_count{result="success"} 1"#,
    );
}

// ─── Test 7: duration on error ───────────────────────────────────────────────

/// A failed conversion (empty body) records one observation in
/// `docspec_conversion_duration_seconds` with `result="client_error"`.
#[test]
fn conversion_duration_recorded_on_error() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(
        &rendered,
        r#"docspec_conversion_duration_seconds_count{result="client_error"} 1"#,
    );
}

// ─── Test 8: body size ───────────────────────────────────────────────────────

/// The body size histogram records the exact byte count of the request body.
/// `b"# Hello"` is 7 bytes (`#`, ` `, `H`, `e`, `l`, `l`, `o`), so the
/// `docspec_http_request_body_bytes_sum` line must equal `7`.
#[test]
fn body_size_histogram_records_correct_byte_count() {
    let rt = common::runtime();
    let (recorder, handle) = docspec_http::metrics::build_recorder().expect("builds");
    let router = docspec_http::router::router_with_metrics(handle.clone());

    // "# Hello" = '#' + ' ' + 'H' + 'e' + 'l' + 'l' + 'o' = 7 bytes
    metrics::with_local_recorder(&recorder, || {
        rt.block_on(router.oneshot(common::accepted_markdown_request("# Hello")))
    })
    .expect("oneshot succeeds");

    let rendered = handle.render();
    assert_metric_line(&rendered, "docspec_http_request_body_bytes_sum 7");
}
