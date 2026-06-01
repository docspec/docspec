//! Integration tests for the HTTP metrics middleware.
//!
//! Each test installs an isolated recorder via [`common::with_test_recorder`]
//! and sends requests using [`tower::ServiceExt::oneshot`].
//!
//! `metrics::with_local_recorder` is thread-local, so tests use a manually
//! created `current_thread` runtime and call `rt.block_on(...)` **inside**
//! the closure so the recorder is active for the entire request lifecycle.

// Reason: integration tests use standard test patterns with expect/unwrap.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use axum::{body::Body, http::StatusCode};
use docspec_http::{
    metrics::{
        HTTP_LATENCY_BUCKETS, METRIC_HTTP_REQUESTS_TOTAL, METRIC_HTTP_REQUEST_DURATION_SECONDS,
    },
    router::router_with_metrics,
};
use tower::ServiceExt as _;

fn parse_metric_lines<'a>(rendered: &'a str, metric_name: &str) -> Vec<&'a str> {
    rendered
        .lines()
        .filter(|l| l.starts_with(metric_name))
        .collect()
}

#[test]
fn get_health_increments_requests_total() {
    let rt = common::runtime();
    let router = common::router();
    let request = common::empty_request("GET", "/health");

    let (handle, _) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    let rendered = handle.render();
    let expected = r#"docspec_http_requests_total{method="GET",path="/health",status="200"} 1"#;
    assert!(
        rendered.lines().any(|l| l == expected),
        "Expected line not found in rendered output:\n{rendered}"
    );
}

#[test]
fn post_conversion_increments_requests_total() {
    let rt = common::runtime();
    let router = common::router();
    let request = common::markdown_request("# Hello");

    let (handle, _) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    let rendered = handle.render();
    let expected =
        r#"docspec_http_requests_total{method="POST",path="/conversion",status="200"} 1"#;
    assert!(
        rendered.lines().any(|l| l == expected),
        "Expected line not found in rendered output:\n{rendered}"
    );
}

#[test]
fn post_conversion_error_status_label() {
    let rt = common::runtime();
    let router = common::router();
    // Empty body → 400 Bad Request from the conversion handler.
    let request = common::markdown_request(Body::empty());

    let (handle, response) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "expected 400 for empty body"
    );

    let rendered = handle.render();
    let expected =
        r#"docspec_http_requests_total{method="POST",path="/conversion",status="400"} 1"#;
    assert!(
        rendered.lines().any(|l| l == expected),
        "Expected status=400 counter line not found in rendered output:\n{rendered}"
    );
}

#[test]
fn request_duration_histogram_records_observation() {
    let rt = common::runtime();
    let router = common::router();
    let request = common::empty_request("GET", "/health");

    let (handle, _) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    let rendered = handle.render();
    let expected = r#"docspec_http_request_duration_seconds_count{method="GET",path="/health",status="200"} 1"#;
    assert!(
        rendered.lines().any(|l| l == expected),
        "Expected histogram _count line not found in rendered output:\n{rendered}"
    );
}

#[test]
fn histogram_buckets_match_spec() {
    let rt = common::runtime();
    let router = common::router();
    let request = common::empty_request("GET", "/health");

    let (handle, _) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    let rendered = handle.render();
    let bucket_prefix = format!("{METRIC_HTTP_REQUEST_DURATION_SECONDS}_bucket");

    // Collect the le= string values from every _bucket line.
    let le_strings: Vec<String> = rendered
        .lines()
        .filter(|l| l.starts_with(bucket_prefix.as_str()))
        .filter_map(|l| {
            let (_, after_le) = l.split_once(r#"le=""#)?;
            let (le_value, _) = after_le.split_once('"')?;
            Some(le_value.to_owned())
        })
        .collect();

    // +Inf must always be present.
    assert!(
        le_strings.iter().any(|s| s == "+Inf"),
        "Expected +Inf bucket; got le= values: {le_strings:?}"
    );

    // Parse finite le= values as f64 and compare numerically against the
    // spec to avoid string-formatting sensitivity (e.g. "1" vs "1.0").
    let mut observed: Vec<f64> = le_strings
        .iter()
        .filter(|s| s.as_str() != "+Inf")
        .map(|s| s.parse::<f64>().expect("le value is a valid float"))
        .collect();
    observed.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut expected: Vec<f64> = HTTP_LATENCY_BUCKETS.to_vec();
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(
        observed, expected,
        "Histogram bucket boundaries don't match HTTP_LATENCY_BUCKETS"
    );
}

#[test]
fn metrics_route_skipped() {
    let rt = common::runtime();
    // Use an explicit recorder/handle pair so the router's /metrics route is
    // properly registered (needed for MatchedPath to be "/metrics").
    let (_, dummy_handle) = docspec_http::metrics::build_recorder().expect("dummy recorder");
    let router = router_with_metrics(dummy_handle);

    let health_req = common::empty_request("GET", "/health");
    let metrics_req1 = common::empty_request("GET", "/metrics");
    let metrics_req2 = common::empty_request("GET", "/metrics");

    let (handle, ()) = common::with_test_recorder(|| {
        let _ = rt
            .block_on(router.clone().oneshot(health_req))
            .expect("health request succeeds");
        let _ = rt
            .block_on(router.clone().oneshot(metrics_req1))
            .expect("first metrics request succeeds");
        let _ = rt
            .block_on(router.clone().oneshot(metrics_req2))
            .expect("second metrics request succeeds");
    });

    let rendered = handle.render();
    let counter_lines = parse_metric_lines(&rendered, METRIC_HTTP_REQUESTS_TOTAL);

    let health_expected =
        r#"docspec_http_requests_total{method="GET",path="/health",status="200"} 1"#;
    assert!(
        counter_lines.contains(&health_expected),
        "Expected /health counter line; rendered output:\n{rendered}"
    );

    assert!(
        !counter_lines
            .iter()
            .any(|l| l.contains(r#"path="/metrics""#)),
        r#"Unexpected path="/metrics" found in counter lines; rendered output:{rendered}"#
    );
}

#[test]
fn fallback_404_uses_unknown_path_label() {
    let rt = common::runtime();
    let router = common::router();
    let request = common::empty_request("GET", "/does-not-exist");

    let (handle, _) = common::with_test_recorder(|| {
        rt.block_on(router.oneshot(request))
            .expect("request succeeds")
    });

    let rendered = handle.render();
    let expected = r#"docspec_http_requests_total{method="GET",path="unknown",status="404"} 1"#;
    assert!(
        rendered.lines().any(|l| l == expected),
        "Expected line not found in rendered output:\n{rendered}"
    );
}
