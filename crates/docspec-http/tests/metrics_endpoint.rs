//! Integration tests for the `/metrics` endpoint.
#![allow(
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use axum::{
    body::{to_bytes, Body},
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
use docspec_http::{
    metrics::{
        build_recorder, METRIC_CONVERSIONS_TOTAL, METRIC_CONVERSION_DURATION_SECONDS,
        METRIC_HTTP_REQUESTS_TOTAL, METRIC_HTTP_REQUEST_BODY_BYTES,
        METRIC_HTTP_REQUEST_DURATION_SECONDS,
    },
    router::router_with_metrics,
};
use metrics::{describe_counter, describe_histogram};
use tower::ServiceExt as _;

#[tokio::test]
async fn get_metrics_returns_200() {
    let (_recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle);
    let request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.expect("oneshot succeeds");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_metrics_content_type_is_prometheus_text() {
    let (_recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle);
    let request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.expect("oneshot succeeds");
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .expect("content-type header present")
        .to_str()
        .expect("content-type is valid UTF-8");
    assert_eq!(content_type, "text/plain; version=0.0.4; charset=utf-8");
}

#[tokio::test]
async fn get_metrics_body_contains_help_and_type_lines_for_all_five_metrics() {
    let (recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle);

    // build_recorder() does not call describe_* — only install_global() does.
    // Register descriptions AND record a data point for each metric:
    // metrics-exporter-prometheus 0.18 only emits # HELP / # TYPE lines for
    // metrics that have at least one recorded data point.
    metrics::with_local_recorder(&recorder, || {
        describe_counter!(METRIC_HTTP_REQUESTS_TOTAL, "Total HTTP requests received.");
        describe_histogram!(
            METRIC_HTTP_REQUEST_DURATION_SECONDS,
            "HTTP request latency in seconds."
        );
        describe_histogram!(
            METRIC_HTTP_REQUEST_BODY_BYTES,
            "HTTP request body size in bytes."
        );
        describe_counter!(METRIC_CONVERSIONS_TOTAL, "Total document conversions.");
        describe_histogram!(
            METRIC_CONVERSION_DURATION_SECONDS,
            "Document conversion duration in seconds."
        );
        metrics::counter!(METRIC_HTTP_REQUESTS_TOTAL).increment(1);
        metrics::histogram!(METRIC_HTTP_REQUEST_DURATION_SECONDS).record(0.001);
        metrics::histogram!(METRIC_HTTP_REQUEST_BODY_BYTES).record(100.0);
        metrics::counter!(METRIC_CONVERSIONS_TOTAL).increment(1);
        metrics::histogram!(METRIC_CONVERSION_DURATION_SECONDS).record(0.001);
    });

    let request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.expect("oneshot succeeds");

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    let lines: Vec<&str> = body.lines().collect();

    for metric_name in [
        METRIC_HTTP_REQUESTS_TOTAL,
        METRIC_HTTP_REQUEST_DURATION_SECONDS,
        METRIC_HTTP_REQUEST_BODY_BYTES,
        METRIC_CONVERSIONS_TOTAL,
        METRIC_CONVERSION_DURATION_SECONDS,
    ] {
        let help_prefix = format!("# HELP {metric_name} ");
        assert!(
            lines.iter().any(|line| line.starts_with(&help_prefix)),
            "Missing # HELP line for metric '{metric_name}'"
        );
        let type_prefix = format!("# TYPE {metric_name} ");
        assert!(
            lines.iter().any(|line| line.starts_with(&type_prefix)),
            "Missing # TYPE line for metric '{metric_name}'"
        );
    }
}

#[tokio::test]
async fn get_metrics_body_contains_no_request_id_label_keys() {
    let (_recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle);
    let request = Request::builder()
        .method("GET")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.expect("oneshot succeeds");

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    for forbidden in ["x_request_id", "x_trace_id", "request_id", "trace_id"] {
        assert!(
            !body.contains(forbidden),
            "Metrics body must not contain label key '{forbidden}'"
        );
    }
}

#[tokio::test]
async fn delete_metrics_returns_405() {
    let (_recorder, handle) = build_recorder().expect("recorder builds");
    let router = router_with_metrics(handle);
    let request = Request::builder()
        .method("DELETE")
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.expect("oneshot succeeds");
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
